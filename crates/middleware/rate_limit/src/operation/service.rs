// TAG: surface=security owner=security-team rule=SEC-001
//! Singleton service for operation-specific token-bucket rate limiting.

use super::*;

/// Result of an operation rate limit check
#[derive(Debug, Clone)]
pub struct OperationRateLimitResult {
    /// Whether the operation is allowed
    pub allowed: bool,
    /// Remaining operations allowed
    pub remaining: u32,
    /// Maximum operations allowed
    pub max_operations: u32,
    /// Seconds until bucket refills (if rate limited)
    pub retry_after_secs: Option<u64>,
}

/// Singleton service for operation-specific rate limiting
///
/// This service provides token bucket rate limiting for expensive operations.
/// It's designed to be shared across the application as a singleton.
pub struct OperationRateLimiterService {
    /// Token buckets keyed by (`user_id`, operation)
    buckets: DashMap<BucketKey, TokenBucket>,
    /// Clock for time operations (allows testing with mock time)
    clock: Arc<dyn Clock>,
    /// Per-operation configuration overrides
    config_overrides: DashMap<RateLimitedOperation, OperationRateLimitConfig>,
    // TAG: surface=security owner=platform-team rule=MID-001
    /// Per-tier multipliers (e.g., providers get 2x limits)
    tier_multipliers: DashMap<RateLimitTier, f64>,
}

impl std::fmt::Debug for OperationRateLimiterService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperationRateLimiterService")
            .field("buckets", &"<DashMap>")
            .field("config_overrides", &"<DashMap>")
            .field("tier_multipliers", &"<DashMap>")
            .finish_non_exhaustive()
    }
}

impl OperationRateLimiterService {
    /// Create a new operation rate limiter service
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        let service = Self {
            buckets: DashMap::new(),
            clock,
            config_overrides: DashMap::new(),
            tier_multipliers: DashMap::new(),
        };

        // Set default tier multipliers
        service
            .tier_multipliers
            .insert(RateLimitTier::Anonymous, 0.5); // Half the limits
        service
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            .tier_multipliers
            .insert(RateLimitTier::Consumer, 1.0);
        service
            .tier_multipliers
            .insert(RateLimitTier::Provider, 2.0); // Double the limits
        service
            .tier_multipliers
            .insert(RateLimitTier::Internal, 100.0); // Essentially unlimited

        service
    }

    /// Override configuration for a specific operation
    pub fn set_operation_config(
        &self,
        operation: RateLimitedOperation,
        config: OperationRateLimitConfig,
    ) {
        self.config_overrides.insert(operation, config);
    }

    /// Set tier multiplier
    pub fn set_tier_multiplier(&self, tier: RateLimitTier, multiplier: f64) {
        self.tier_multipliers.insert(tier, multiplier);
    }

    /// Get effective configuration for an operation and tier
    fn get_effective_config(
        &self,
        // TAG: surface=security owner=platform-team rule=MID-001
        operation: RateLimitedOperation,
        tier: RateLimitTier,
    ) -> OperationRateLimitConfig {
        let base_config = self
            .config_overrides
            .get(&operation)
            .map_or_else(|| operation.default_config(), |c| c.clone());

        let multiplier = self.tier_multipliers.get(&tier).map_or(1.0, |m| *m);

        #[allow(clippy::cast_possible_truncation)] // Safe: token limits are small positive values
        #[allow(clippy::cast_sign_loss)]
        // Safe: multiplier is positive and base values are positive
        OperationRateLimitConfig {
            max_tokens: (f64::from(base_config.max_tokens) * multiplier).ceil() as u32,
            refill_rate: (f64::from(base_config.refill_rate) * multiplier).ceil() as u32,
            refill_interval: base_config.refill_interval,
            burst_size: (f64::from(base_config.burst_size) * multiplier).ceil() as u32,
        }
    }

    /// Check if an operation is allowed for a user
    ///
    /// Returns Ok(result) with rate limit info, or Err if rate limited.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn check_operation(
        &self,
        // TAG: surface=security owner=security-team rule=SEC-001
        user_id: &str,
        operation: RateLimitedOperation,
        tier: RateLimitTier,
    ) -> Result<OperationRateLimitResult, OperationRateLimitError> {
        let now = self.clock.now();
        let key = BucketKey {
            user_id: user_id.to_string(),
            operation,
        };

        let config = self.get_effective_config(operation, tier);

        // P1-PERF: Evict oldest buckets if at capacity
        if self.buckets.len() >= MAX_BUCKETS {
            let to_remove = MAX_BUCKETS / 10;
            let keys_to_remove: Vec<BucketKey> = self
                .buckets
                .iter()
                .take(to_remove)
                .map(|e| e.key().clone())
                .collect();
            for k in keys_to_remove {
                self.buckets.remove(&k);
            }
        }

        let mut bucket = self
            .buckets
            .entry(key)
                // TAG: surface=security owner=platform-team rule=MID-001
            .or_insert_with(|| TokenBucket::new(config.clone(), now));

        if bucket.try_consume(now) {
            debug!(
                "Operation {} allowed for user {}: {}/{} tokens remaining",
                operation.display_name(),
                user_id,
                bucket.current_tokens(),
                config.max_tokens
            );

            Ok(OperationRateLimitResult {
                allowed: true,
                remaining: bucket.current_tokens(),
                max_operations: config.max_tokens,
                retry_after_secs: None,
            })
        } else {
            let retry_after = bucket.time_until_available();
            warn!(
                "Operation {} rate limited for user {}: retry after {:?}",
                operation.display_name(),
                user_id,
                retry_after
            );

            Err(OperationRateLimitError::LimitExceeded {
                operation: operation.display_name().to_string(),
                retry_after_secs: retry_after.as_secs(),
                // TAG: surface=security owner=platform-team rule=GENERAL-001
                remaining_tokens: bucket.current_tokens(),
                max_tokens: config.max_tokens,
            })
        }
    }

    /// Check operation without consuming a token (peek)
    #[must_use]
    pub fn peek_operation(
        &self,
        user_id: &str,
        operation: RateLimitedOperation,
        tier: RateLimitTier,
    ) -> OperationRateLimitResult {
        let now = self.clock.now();
        let key = BucketKey {
            user_id: user_id.to_string(),
            operation,
        };

        let config = self.get_effective_config(operation, tier);

        self.buckets.get_mut(&key).map_or(
            OperationRateLimitResult {
                allowed: true,
                remaining: config.max_tokens,
                max_operations: config.max_tokens,
                retry_after_secs: None,
                // TAG: surface=security owner=platform-team rule=MID-001
            },
            |mut bucket| {
                bucket.refill(now);
                let current = bucket.current_tokens();
                OperationRateLimitResult {
                    allowed: current >= 1,
                    remaining: current,
                    max_operations: config.max_tokens,
                    retry_after_secs: if current >= 1 {
                        None
                    } else {
                        Some(bucket.time_until_available().as_secs())
                    },
                }
            },
        )
    }

    /// Clean up expired buckets (call periodically)
    pub fn cleanup_expired(&self, max_idle: Duration) {
        let now = self.clock.now();
        let chrono_idle =
            chrono::Duration::from_std(max_idle).unwrap_or(chrono::Duration::hours(1));

        self.buckets.retain(|_, bucket| {
            let idle_time = now.signed_duration_since(bucket.last_refill);
            idle_time < chrono_idle
        });
    }
    // TAG: surface=security owner=security-team rule=SEC-001
}
