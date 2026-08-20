// TAG: surface=security owner=security-team rule=SEC-001
//! Token bucket for operation-specific rate limiting.

use super::*;

/// Token bucket state for a single user+operation combination
#[derive(Debug, Clone)]
pub struct TokenBucket {
    /// Current number of tokens
    pub tokens: f64,
    /// Last refill time
    pub last_refill: DateTime<Utc>,
    /// Configuration
    pub config: OperationRateLimitConfig,
}

impl TokenBucket {
    /// Create a new token bucket.
    #[must_use]
    pub const fn new(config: OperationRateLimitConfig, now: DateTime<Utc>) -> Self {
        Self {
            tokens: config.max_tokens as f64,
            last_refill: now,
            config,
        }
    }

    /// Refill tokens based on elapsed time
    #[allow(clippy::cast_precision_loss)] // Safe: ms-level precision is sufficient for token buckets
    pub fn refill(&mut self, now: DateTime<Utc>) {
        let elapsed = now.signed_duration_since(self.last_refill);
        if elapsed.num_milliseconds() <= 0 {
            return;
        }

        let interval_ms = self.config.refill_interval.as_millis() as f64;
        let intervals = elapsed.num_milliseconds() as f64 / interval_ms;
        let tokens_to_add = intervals * f64::from(self.config.refill_rate);

        self.tokens = (self.tokens + tokens_to_add).min(f64::from(self.config.max_tokens));
        self.last_refill = now;
    }

    // TAG: surface=security owner=platform-team rule=MID-001
    /// Try to consume a token, returns true if successful
    pub fn try_consume(&mut self, now: DateTime<Utc>) -> bool {
        self.refill(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Get time until next token is available
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Safe: ms-level precision is sufficient for rate-limit waits
    #[allow(clippy::cast_possible_truncation)] // Safe: ms_needed is positive and fits in u64
    #[allow(clippy::cast_sign_loss)] // Safe: ms_needed is positive
    pub fn time_until_available(&self) -> Duration {
        if self.tokens >= 1.0 {
            return Duration::ZERO;
        }
        let tokens_needed = 1.0 - self.tokens;
        let intervals_needed = tokens_needed / f64::from(self.config.refill_rate);
        let ms_needed = intervals_needed * self.config.refill_interval.as_millis() as f64;
        Duration::from_millis(ms_needed.ceil() as u64)
    }

    /// Get current token count
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // Safe: tokens are bounded by max_tokens (u32)
    #[allow(clippy::cast_sign_loss)] // Safe: tokens are always non-negative
    pub fn current_tokens(&self) -> u32 {
        self.tokens.floor() as u32
    }
}

/// Composite key for user+operation buckets
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BucketKey {
    /// User identifier.
    pub user_id: String,
    /// The rate-limited operation.
    pub operation: RateLimitedOperation,
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
