// TAG: surface=security owner=security-team rule=SEC-001
//! Rate limit configuration

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Rate limit tier for different user types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RateLimitTier {
    /// Anonymous/unauthenticated users
    Anonymous,
    /// Authenticated consumers
    Consumer,
    /// Authenticated providers
    Provider,
    /// Internal services (no rate limit)
    Internal,
}

impl RateLimitTier {
    /// Get the maximum requests allowed for this tier
    #[must_use]
    pub const fn max_requests(&self) -> u32 {
        match self {
            // Anonymous: 200 req/min - safety buffer for unauthenticated browsing
            // Note: Most page routes are now exempt, so this mainly affects API calls
            Self::Anonymous => 200,
            Self::Consumer => 300,      // 300 requests per minute
            Self::Provider => 600,      // 600 requests per minute
            Self::Internal => u32::MAX, // No limit
        }
    }

    /// Get the time window for this tier
    #[must_use]
    pub const fn window_duration(&self) -> Duration {
        match self {
            // All external tiers use a 1-minute window
            Self::Anonymous | Self::Consumer | Self::Provider => Duration::from_secs(60),
            Self::Internal => Duration::from_secs(1), // Irrelevant
        }
    }
}

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests allowed in the time window
    pub max_requests: u32,
    // TAG: surface=security owner=platform-team rule=MID-001
    /// Time window for rate limiting
    pub window_duration: Duration,

    /// Whether to include rate limit headers in responses
    pub include_headers: bool,
}

impl RateLimitConfig {
    /// Create a new rate limit configuration
    #[must_use]
    pub const fn new(max_requests: u32, window_duration: Duration) -> Self {
        Self {
            max_requests,
            window_duration,
            include_headers: true,
        }
    }

    /// Create configuration from tier
    #[must_use]
    pub const fn from_tier(tier: RateLimitTier) -> Self {
        Self {
            max_requests: tier.max_requests(),
            window_duration: tier.window_duration(),
            include_headers: true,
            // TAG: surface=security owner=platform-team rule=MID-001
        }
    }

    /// Create configuration for anonymous users
    #[must_use]
    pub const fn anonymous() -> Self {
        Self::from_tier(RateLimitTier::Anonymous)
    }

    /// Create configuration for authenticated consumers
    #[must_use]
    pub const fn consumer() -> Self {
        Self::from_tier(RateLimitTier::Consumer)
    }

    /// Create configuration for authenticated providers
    #[must_use]
    pub const fn provider() -> Self {
        Self::from_tier(RateLimitTier::Provider)
    }

    /// Create configuration for internal services (no limit)
    #[must_use]
    pub const fn internal() -> Self {
        Self::from_tier(RateLimitTier::Internal)
    }
    // TAG: surface=security owner=platform-team rule=MID-001

    /// Create configuration for authentication endpoints
    /// SECURITY: Stricter limits to prevent brute force attacks
    /// 100 requests per 5 minutes per IP for auth endpoints
    /// NOTE: Increased from 20/5min to 100/5min to accommodate OAuth flow
    /// OAuth requires: login page (1) + start OAuth (1) + callback (1) + potential retries
    /// Plus: Page reloads, navigation, and testing during development
    #[must_use]
    pub const fn auth() -> Self {
        Self {
            max_requests: 100,                         // 100 attempts
            window_duration: Duration::from_secs(300), // 5 minutes
            include_headers: true,
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self::anonymous()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // TAG: surface=security owner=platform-team rule=MID-001

    #[test]
    fn test_rate_limit_tier_anonymous() {
        let tier = RateLimitTier::Anonymous;
        assert_eq!(tier.max_requests(), 200);
        assert_eq!(tier.window_duration(), Duration::from_secs(60));
    }

    #[test]
    fn test_rate_limit_tier_consumer() {
        let tier = RateLimitTier::Consumer;
        assert_eq!(tier.max_requests(), 300);
        assert_eq!(tier.window_duration(), Duration::from_secs(60));
    }

    #[test]
    fn test_rate_limit_tier_provider() {
        let tier = RateLimitTier::Provider;
        assert_eq!(tier.max_requests(), 600);
        assert_eq!(tier.window_duration(), Duration::from_secs(60));
    }

    #[test]
    fn test_rate_limit_tier_internal() {
        let tier = RateLimitTier::Internal;
        assert_eq!(tier.max_requests(), u32::MAX);
        // TAG: surface=security owner=platform-team rule=MID-001
    }

    #[test]
    fn test_rate_limit_config_new() {
        let config = RateLimitConfig::new(100, Duration::from_secs(30));
        assert_eq!(config.max_requests, 100);
        assert_eq!(config.window_duration, Duration::from_secs(30));
        assert!(config.include_headers);
    }

    #[test]
    fn test_rate_limit_config_from_tier() {
        let config = RateLimitConfig::from_tier(RateLimitTier::Consumer);
        assert_eq!(config.max_requests, 300);
        assert_eq!(config.window_duration, Duration::from_secs(60));
    }

    #[test]
    fn test_rate_limit_config_anonymous() {
        let config = RateLimitConfig::anonymous();
        assert_eq!(config.max_requests, 200);
    }

    #[test]
    fn test_rate_limit_config_consumer() {
        let config = RateLimitConfig::consumer();
        // TAG: surface=security owner=platform-team rule=MID-001
        assert_eq!(config.max_requests, 300);
    }

    #[test]
    fn test_rate_limit_config_provider() {
        let config = RateLimitConfig::provider();
        assert_eq!(config.max_requests, 600);
    }

    #[test]
    fn test_rate_limit_config_internal() {
        let config = RateLimitConfig::internal();
        assert_eq!(config.max_requests, u32::MAX);
    }

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.max_requests, 200); // Anonymous tier
    }

    #[test]
    fn test_rate_limit_tier_equality() {
        assert_eq!(RateLimitTier::Provider, RateLimitTier::Provider);
        assert_ne!(RateLimitTier::Provider, RateLimitTier::Consumer);
    }
}
