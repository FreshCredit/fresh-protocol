// TAG: surface=security owner=security-team rule=SEC-001
//! Idempotency configuration

use std::time::Duration;

/// Idempotency configuration
#[derive(Debug, Clone)]
pub struct IdempotencyConfig {
    /// TTL for idempotency keys (how long to store responses)
    pub ttl: Duration,

    /// Header name for idempotency key
    pub header_name: String,

    /// Whether idempotency key is required
    pub required: bool,

    /// Maximum size of stored response body (in bytes)
    pub max_body_size: usize,

    /// How long an `in_flight` durable claim may be held before another
    /// caller may reclaim it (crash recovery for the durable store)
    pub stale_seconds: u64,
}

impl IdempotencyConfig {
    /// Create a new idempotency configuration
    #[must_use]
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            header_name: "Idempotency-Key".to_string(),
            required: false,
            // TAG: surface=security owner=platform-team rule=MID-001
            max_body_size: 1024 * 1024, // 1 MB
            stale_seconds: Self::default_stale_seconds(),
        }
    }

    /// Create configuration with required idempotency key
    #[must_use]
    pub fn required(ttl: Duration) -> Self {
        Self {
            ttl,
            header_name: "Idempotency-Key".to_string(),
            required: true,
            max_body_size: 1024 * 1024, // 1 MB
            stale_seconds: Self::default_stale_seconds(),
        }
    }

    /// Default staleness timeout for in-flight durable claims (5 minutes)
    #[must_use]
    pub const fn default_stale_seconds() -> u64 {
        300
    }

    /// Set custom header name
    #[must_use]
    pub fn with_header_name(mut self, header_name: String) -> Self {
        self.header_name = header_name;
        self
    }

    /// Set maximum body size
    #[must_use]
    pub const fn with_max_body_size(mut self, max_body_size: usize) -> Self {
        self.max_body_size = max_body_size;
        self
    }

    /// Set how long an in-flight durable claim may be held before it can be
    /// reclaimed by another caller (durable store crash recovery)
    #[must_use]
    pub const fn with_stale_seconds(mut self, stale_seconds: u64) -> Self {
        self.stale_seconds = stale_seconds;
        self
    }
    // TAG: surface=security owner=platform-team rule=MID-001
}

impl Default for IdempotencyConfig {
    fn default() -> Self {
        Self::new(Duration::from_secs(24 * 60 * 60)) // 24 hours
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idempotency_config_new() {
        let config = IdempotencyConfig::new(Duration::from_secs(3600));
        assert_eq!(config.ttl, Duration::from_secs(3600));
        assert_eq!(config.header_name, "Idempotency-Key");
        assert!(!config.required);
        assert_eq!(config.max_body_size, 1024 * 1024);
    }

    #[test]
    fn test_idempotency_config_required() {
        let config = IdempotencyConfig::required(Duration::from_secs(7200));
        assert_eq!(config.ttl, Duration::from_secs(7200));
        assert!(config.required);
    }

    #[test]
    fn test_idempotency_config_with_header_name() {
        // TAG: surface=security owner=platform-team rule=MID-001
        let config = IdempotencyConfig::new(Duration::from_secs(3600))
            .with_header_name("X-Request-Id".to_string());
        assert_eq!(config.header_name, "X-Request-Id");
    }

    #[test]
    fn test_idempotency_config_with_max_body_size() {
        let config =
            IdempotencyConfig::new(Duration::from_secs(3600)).with_max_body_size(512 * 1024);
        assert_eq!(config.max_body_size, 512 * 1024);
    }

    #[test]
    fn test_idempotency_config_default() {
        let config = IdempotencyConfig::default();
        assert_eq!(config.ttl, Duration::from_secs(24 * 60 * 60));
        assert_eq!(config.header_name, "Idempotency-Key");
        assert!(!config.required);
    }

    #[test]
    fn test_idempotency_config_builder_chain() {
        let config = IdempotencyConfig::new(Duration::from_secs(1800))
            .with_header_name("X-Idempotency".to_string())
            .with_max_body_size(2 * 1024 * 1024);

        assert_eq!(config.ttl, Duration::from_secs(1800));
        assert_eq!(config.header_name, "X-Idempotency");
        assert_eq!(config.max_body_size, 2 * 1024 * 1024);
    }

    #[test]
    fn test_idempotency_config_stale_seconds_default() {
        let config = IdempotencyConfig::new(Duration::from_secs(3600));
        assert_eq!(
            config.stale_seconds,
            IdempotencyConfig::default_stale_seconds()
        );
        assert_eq!(config.stale_seconds, 300);
    }

    #[test]
    fn test_idempotency_config_with_stale_seconds() {
        let config = IdempotencyConfig::new(Duration::from_secs(3600)).with_stale_seconds(30);
        assert_eq!(config.stale_seconds, 30);

        let required = IdempotencyConfig::required(Duration::from_secs(3600));
        assert_eq!(required.stale_seconds, 300);
    }
}
