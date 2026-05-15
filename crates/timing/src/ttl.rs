//! TTL (Time-To-Live) enforcement for ephemeral data
//!
//! This module provides traits and utilities for enforcing TTL on ephemeral data
//! such as sessions, CSRF tokens, rate limits, etc.

use async_trait::async_trait;
use chrono::{
    DateTime,
    Duration,
    Utc,
};
use serde::{
    Deserialize,
    Serialize,
};

/// TTL enforcement trait for ephemeral data
#[async_trait]
pub trait TtlEnforcement: Send + Sync {
    /// Check if item has expired based on TTL
    fn is_expired(&self, created_at: DateTime<Utc>, ttl: Duration) -> bool {
        Utc::now() > created_at + ttl
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
pub enum TtlError {
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Invalid TTL: {0}")]
    InvalidTtl(String),
}

/// TTL configuration for different data types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlConfig {
    pub sessions: Duration,
    pub csrf_tokens: Duration,
    pub rate_limits: Duration,
    pub staged_payloads: Duration,
    pub idempotency_keys: Duration,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            sessions: Duration::hours(24),
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
    use super::*;
    use std::sync::Mutex;

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
        assert_eq!(config.rate_limits, Duration::hours(2));
        assert_eq!(config.staged_payloads, Duration::hours(24));
        assert_eq!(config.idempotency_keys, Duration::hours(12));

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
}
