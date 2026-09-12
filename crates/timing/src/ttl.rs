// TAG: surface=api owner=platform-team rule=API-001
//! TTL (Time-To-Live) enforcement for ephemeral data
//!
//! This module provides traits and utilities for enforcing TTL on ephemeral data
//! such as sessions, CSRF tokens, rate limits, etc.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::clock::Clock;

/// TTL enforcement trait for ephemeral data
#[async_trait]
pub trait TtlEnforcement: Send + Sync {
    /// Check if item has expired based on TTL using the provided clock.
    fn is_expired_with_clock(
        &self,
        clock: &dyn Clock,
        created_at: DateTime<Utc>,
        ttl: Duration,
    ) -> bool {
        clock.now() > created_at + ttl
    }

    /// Check if item has expired based on TTL using the system wall-clock.
    fn is_expired(&self, created_at: DateTime<Utc>, ttl: Duration) -> bool {
        self.is_expired_with_clock(&crate::clock::SystemClock, created_at, ttl)
    }

    /// Calculate expiration timestamp
    fn expiration_timestamp(&self, created_at: DateTime<Utc>, ttl: Duration) -> DateTime<Utc> {
        created_at + ttl
    }

    /// Cleanup expired items (implemented by storage layer)
    async fn cleanup_expired(&self, table: &str, ttl: Duration) -> Result<u64, TtlError>;
}

/// TTL-related errors
#[derive(Debug, thiserror::Error)]
// TAG: surface=api owner=platform-team rule=API-001
pub enum TtlError {
    #[error("Database error: {0}")]
    /// Databaseerror
    DatabaseError(String),

    #[error("Invalid TTL: {0}")]
    /// Invalidttl
    InvalidTtl(String),
}

/// TTL configuration for different data types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlConfig {
    /// Sessions
    pub sessions: Duration,
    /// Csrf Tokens
    pub csrf_tokens: Duration,
    /// Rate Limits
    pub rate_limits: Duration,
    /// Staged Payloads
    pub staged_payloads: Duration,
    /// Idempotency Keys
    pub idempotency_keys: Duration,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            sessions: Duration::hours(24),
            // TAG: surface=api owner=platform-team rule=API-001
            csrf_tokens: Duration::minutes(30),
            rate_limits: Duration::hours(1),
            staged_payloads: Duration::hours(48),
            idempotency_keys: Duration::hours(24),
        }
    }
}

impl TtlConfig {
    /// Create TTL config from environment variables
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            sessions: Duration::seconds(
                std::env::var("TTL_SESSION_HOURS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(24)
                    * 3600,
            ),
            csrf_tokens: Duration::seconds(
                std::env::var("TTL_CSRF_MINUTES")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(30)
                    * 60,
            ),
            rate_limits: Duration::seconds(
                // TAG: surface=api owner=platform-team rule=API-001
                std::env::var("TTL_RATE_LIMIT_HOURS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(1)
                    * 3600,
            ),
            staged_payloads: Duration::seconds(
                std::env::var("TTL_STAGED_PAYLOAD_HOURS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(48)
                    * 3600,
            ),
            idempotency_keys: Duration::seconds(
                std::env::var("TTL_IDEMPOTENCY_KEY_HOURS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(24)
                    * 3600,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)]
    use super::*;
    use std::sync::Mutex;
    // TAG: surface=api owner=platform-team rule=API-001

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_ttl_config_default() {
        let config = TtlConfig::default();
        assert_eq!(config.sessions, Duration::hours(24));
        assert_eq!(config.csrf_tokens, Duration::minutes(30));
        assert_eq!(config.rate_limits, Duration::hours(1));
        assert_eq!(config.staged_payloads, Duration::hours(48));
        assert_eq!(config.idempotency_keys, Duration::hours(24));
    }

    #[test]
    fn test_expiration_calculation() {
        struct TestTtl;

        #[async_trait]
        impl TtlEnforcement for TestTtl {
            async fn cleanup_expired(&self, _table: &str, _ttl: Duration) -> Result<u64, TtlError> {
                Ok(0)
            }
        }

        let ttl = TestTtl;
        let created_at = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        // TAG: surface=api owner=platform-team rule=API-001
        let ttl_duration = Duration::hours(24);

        let expiration = ttl.expiration_timestamp(created_at, ttl_duration);
        assert_eq!(expiration, created_at + Duration::hours(24));
    }

    #[test]
    fn test_ttl_is_expired() {
        struct TestTtl;

        #[async_trait]
        impl TtlEnforcement for TestTtl {
            async fn cleanup_expired(&self, _table: &str, _ttl: Duration) -> Result<u64, TtlError> {
                Ok(0)
            }
        }

        let ttl = TestTtl;
        let created_at = Utc::now() - Duration::hours(25);
        let ttl_duration = Duration::hours(24);

        assert!(ttl.is_expired(created_at, ttl_duration));
    }

    #[test]
    fn test_ttl_is_not_expired() {
        struct TestTtl;

        #[async_trait]
        // TAG: surface=api owner=platform-team rule=API-001
        impl TtlEnforcement for TestTtl {
            async fn cleanup_expired(&self, _table: &str, _ttl: Duration) -> Result<u64, TtlError> {
                Ok(0)
            }
        }

        let ttl = TestTtl;
        let created_at = Utc::now() - Duration::hours(1);
        let ttl_duration = Duration::hours(24);

        assert!(!ttl.is_expired(created_at, ttl_duration));
    }

    #[test]
    fn test_ttl_config_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: Test-only env manipulation. Guarded by ENV_LOCK mutex.
        unsafe {
            std::env::set_var("TTL_SESSION_HOURS", "12");
            std::env::set_var("TTL_CSRF_MINUTES", "15");
            std::env::set_var("TTL_RATE_LIMIT_HOURS", "2");
            std::env::set_var("TTL_STAGED_PAYLOAD_HOURS", "24");
            std::env::set_var("TTL_IDEMPOTENCY_KEY_HOURS", "12");
        }

        let config = TtlConfig::from_env();
        assert_eq!(config.sessions, Duration::hours(12));
        assert_eq!(config.csrf_tokens, Duration::minutes(15));
        // TAG: surface=api owner=platform-team rule=API-001
        assert_eq!(config.rate_limits, Duration::hours(2));
        assert_eq!(config.staged_payloads, Duration::hours(24));
        assert_eq!(config.idempotency_keys, Duration::hours(12));

        // SAFETY: Test-only env cleanup. Removes vars set above in same test.
        unsafe {
            std::env::remove_var("TTL_SESSION_HOURS");
            std::env::remove_var("TTL_CSRF_MINUTES");
            std::env::remove_var("TTL_RATE_LIMIT_HOURS");
            std::env::remove_var("TTL_STAGED_PAYLOAD_HOURS");
            std::env::remove_var("TTL_IDEMPOTENCY_KEY_HOURS");
        }
    }

    #[tokio::test]
    async fn test_ttl_cleanup_expired() {
        struct TestTtl;

        #[async_trait]
        impl TtlEnforcement for TestTtl {
            async fn cleanup_expired(&self, table: &str, ttl: Duration) -> Result<u64, TtlError> {
                assert_eq!(table, "sessions");
                assert_eq!(ttl, Duration::hours(24));
                Ok(5)
            }
        }

        let ttl = TestTtl;
        let result = ttl.cleanup_expired("sessions", Duration::hours(24)).await;
        // TAG: surface=api owner=platform-team rule=API-001
        assert_eq!(result.unwrap(), 5);
    }

    #[test]
    fn test_ttl_config_from_env_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("TTL_SESSION_HOURS");
        std::env::remove_var("TTL_CSRF_MINUTES");
        std::env::remove_var("TTL_RATE_LIMIT_HOURS");
        std::env::remove_var("TTL_STAGED_PAYLOAD_HOURS");
        std::env::remove_var("TTL_IDEMPOTENCY_KEY_HOURS");

        let config = TtlConfig::from_env();
        assert_eq!(config.sessions, Duration::hours(24));
        assert_eq!(config.csrf_tokens, Duration::minutes(30));
        assert_eq!(config.rate_limits, Duration::hours(1));
        assert_eq!(config.staged_payloads, Duration::hours(48));
        assert_eq!(config.idempotency_keys, Duration::hours(24));
    }

    #[test]
    fn test_ttl_is_expired_boundary() {
        struct TestTtl;

        #[async_trait]
        impl TtlEnforcement for TestTtl {
            async fn cleanup_expired(&self, _table: &str, _ttl: Duration) -> Result<u64, TtlError> {
                Ok(0)
                // TAG: surface=api owner=platform-team rule=API-001
            }
        }

        let ttl = TestTtl;
        // Just over 24 hours old should be expired
        let created_at = Utc::now() - Duration::hours(24) - Duration::seconds(1);
        let ttl_duration = Duration::hours(24);

        assert!(ttl.is_expired(created_at, ttl_duration));
    }

    #[test]
    fn test_ttl_is_not_expired_boundary() {
        struct TestTtl;

        #[async_trait]
        impl TtlEnforcement for TestTtl {
            async fn cleanup_expired(&self, _table: &str, _ttl: Duration) -> Result<u64, TtlError> {
                Ok(0)
            }
        }

        let ttl = TestTtl;
        let created_at = Utc::now();
        let ttl_duration = Duration::hours(24);

        assert!(!ttl.is_expired(created_at, ttl_duration));
    }

    #[test]
    fn test_ttl_is_expired_with_mock_clock() {
        use crate::clock::MockClock;

        struct TestTtl;

        #[async_trait]
        impl TtlEnforcement for TestTtl {
            async fn cleanup_expired(&self, _table: &str, _ttl: Duration) -> Result<u64, TtlError> {
                Ok(0)
            }
        }

        let ttl = TestTtl;
        let base = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let clock = MockClock::new(base);

        // Expired: created 25 hours ago, TTL 24 hours.
        let created_at = base - Duration::hours(25);
        assert!(ttl.is_expired_with_clock(&clock, created_at, Duration::hours(24)));

        // Not expired: created 1 hour ago.
        let created_at = base - Duration::hours(1);
        assert!(!ttl.is_expired_with_clock(&clock, created_at, Duration::hours(24)));
    }
    // TAG: surface=api owner=platform-team rule=API-001
}
