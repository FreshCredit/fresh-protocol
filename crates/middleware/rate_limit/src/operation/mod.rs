// TAG: surface=security owner=security-team rule=SEC-001
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use freshcredit_core_timing::Clock;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

use crate::config::RateLimitTier;

/// Token bucket implementation.
pub mod bucket;
/// Operation rate limiter service.
pub mod service;
pub mod types;

pub use bucket::*;
pub use service::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use freshcredit_core_timing::SystemClock;

    #[test]
    fn test_operation_default_config() {
        let plaid = RateLimitedOperation::PlaidAccountSync;
        // TAG: surface=security owner=platform-team rule=MID-001
        let config = plaid.default_config();
        assert_eq!(config.max_tokens, 2);
        assert_eq!(config.refill_interval, Duration::from_secs(3600));
    }

    #[test]
    fn test_operation_rate_limiter_allows_within_limit() {
        let clock = Arc::new(SystemClock);
        let service = OperationRateLimiterService::new(clock);

        // First call should succeed
        let result = service.check_operation(
            "user1",
            RateLimitedOperation::AgentQuery,
            RateLimitTier::Consumer,
        );
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.allowed);
        assert_eq!(result.max_operations, 30);
    }

    #[test]
    fn test_operation_rate_limiter_blocks_when_exceeded() {
        let clock = Arc::new(SystemClock);
        let service = OperationRateLimiterService::new(clock);

        // TAG: surface=security owner=platform-team rule=MID-001
        // Override to very low limit for testing
        service.set_operation_config(
            RateLimitedOperation::AgentQuery,
            OperationRateLimitConfig {
                max_tokens: 2,
                refill_rate: 1,
                refill_interval: Duration::from_secs(60),
                burst_size: 2,
            },
        );

        // First two calls should succeed
        assert!(service
            .check_operation(
                "user1",
                RateLimitedOperation::AgentQuery,
                RateLimitTier::Consumer
            )
            .is_ok());
        assert!(service
            .check_operation(
                "user1",
                RateLimitedOperation::AgentQuery,
                RateLimitTier::Consumer
            )
            .is_ok());

        // Third call should fail
        // TAG: surface=security owner=platform-team rule=MID-001
        let result = service.check_operation(
            "user1",
            RateLimitedOperation::AgentQuery,
            RateLimitTier::Consumer,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_tier_multiplier() {
        let clock = Arc::new(SystemClock);
        let service = OperationRateLimiterService::new(clock);

        // Provider tier should have 2x limits
        let consumer_result = service.peek_operation(
            "consumer",
            RateLimitedOperation::AgentQuery,
            RateLimitTier::Consumer,
        );
        let provider_result = service.peek_operation(
            "provider",
            RateLimitedOperation::AgentQuery,
            RateLimitTier::Provider,
        );

        assert_eq!(consumer_result.max_operations, 30);
        assert_eq!(provider_result.max_operations, 60); // 2x
    }
}
