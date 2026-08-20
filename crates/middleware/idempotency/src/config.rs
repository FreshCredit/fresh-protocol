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
        }
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
}
