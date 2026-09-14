// TAG: surface=security owner=security-team rule=SEC-001
//! Session configuration

use chrono::Duration;
use serde::{Deserialize, Serialize};

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Session timeout (inactivity timeout)
    pub timeout: Duration,

    /// Maximum session age (absolute timeout)
    pub max_age: Duration,

    /// Cookie name for session ID
    pub cookie_name: String,

    /// Cookie domain
    pub cookie_domain: Option<String>,

    /// Cookie path
    pub cookie_path: String,

    /// Cookie secure flag (HTTPS only)
    pub cookie_secure: bool,

    /// Cookie HTTP-only flag
    pub cookie_http_only: bool,

    /// Cookie `SameSite` policy
    pub cookie_same_site: SameSitePolicy,

    /// Cleanup interval for expired sessions
    pub cleanup_interval: Duration,
}

/// `SameSite` cookie policy
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SameSitePolicy {
    /// Strict `SameSite` policy — cookie is never sent in cross-site requests.
    Strict,
    /// Lax `SameSite` policy — cookie is sent for top-level navigation GET requests.
    Lax,
    // TAG: surface=security owner=platform-team rule=MID-001
    /// No `SameSite` policy — cookie is sent with all requests.
    None,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::minutes(30),
            max_age: Duration::hours(24),
            cookie_name: "freshcredit_session".to_string(),
            cookie_domain: None,
            cookie_path: "/".to_string(),
            cookie_secure: true,
            cookie_http_only: true,
            cookie_same_site: SameSitePolicy::Lax,
            cleanup_interval: Duration::hours(1),
        }
    }
}

impl SessionConfig {
    /// Create session config from environment variables
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            timeout: Duration::minutes(
                std::env::var("SESSION_TIMEOUT_MINUTES")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(30),
            ),
            max_age: Duration::hours(
                std::env::var("SESSION_MAX_AGE_HOURS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(24),
            ),
            cookie_name: std::env::var("SESSION_COOKIE_NAME")
                .unwrap_or_else(|_| "freshcredit_session".to_string()),
            cookie_domain: std::env::var("SESSION_COOKIE_DOMAIN").ok(),
            cookie_path: std::env::var("SESSION_COOKIE_PATH").unwrap_or_else(|_| "/".to_string()),
            cookie_secure: std::env::var("SESSION_COOKIE_SECURE")
                .ok()
                .and_then(|v| v.parse::<bool>().ok())
                .unwrap_or(true),
            cookie_http_only: std::env::var("SESSION_COOKIE_HTTP_ONLY")
                .ok()
                // TAG: surface=security owner=platform-team rule=MID-001
                .and_then(|v| v.parse::<bool>().ok())
                .unwrap_or(true),
            cookie_same_site: match std::env::var("SESSION_COOKIE_SAME_SITE")
                .unwrap_or_else(|_| "Lax".to_string())
                .to_lowercase()
                .as_str()
            {
                "strict" => SameSitePolicy::Strict,
                "none" => SameSitePolicy::None,
                _ => SameSitePolicy::Lax,
            },
            cleanup_interval: Duration::hours(
                std::env::var("SESSION_CLEANUP_INTERVAL_HOURS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(1),
            ),
        }
    }

    /// Validate configuration
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn validate(&self) -> Result<(), String> {
        if self.timeout <= Duration::zero() {
            return Err("Session timeout must be positive".to_string());
        }

        if self.max_age <= Duration::zero() {
            return Err("Session max age must be positive".to_string());
        }

        if self.timeout > self.max_age {
            return Err("Session timeout cannot exceed max age".to_string());
        }

        if self.cleanup_interval <= Duration::zero() {
            return Err("Cleanup interval must be positive".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // TAG: surface=security owner=platform-team rule=MID-001
    use super::*;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_config() {
        let config = SessionConfig::default();
        assert_eq!(config.timeout, Duration::minutes(30));
        assert_eq!(config.max_age, Duration::hours(24));
        assert_eq!(config.cookie_name, "freshcredit_session");
        assert!(config.cookie_secure);
        assert!(config.cookie_http_only);
    }

    #[test]
    fn test_config_validation() {
        let mut config = SessionConfig::default();
        assert!(config.validate().is_ok());

        // Invalid: timeout > max_age
        config.timeout = Duration::hours(48);
        assert!(config.validate().is_err());

        // Invalid: negative timeout
        config.timeout = Duration::minutes(-10);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_negative_max_age() {
        let config = SessionConfig {
            max_age: Duration::minutes(-10),
            ..SessionConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_negative_cleanup_interval() {
        let config = SessionConfig {
            cleanup_interval: Duration::minutes(-10),
            ..SessionConfig::default()
        };
        assert!(config.validate().is_err());
    }
    // TAG: surface=security owner=platform-team rule=MID-001

    #[test]
    fn test_from_env_defaults() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Ensure no relevant env vars are set
        for key in [
            "SESSION_TIMEOUT_MINUTES",
            "SESSION_MAX_AGE_HOURS",
            "SESSION_COOKIE_NAME",
            "SESSION_COOKIE_DOMAIN",
            "SESSION_COOKIE_PATH",
            "SESSION_COOKIE_SECURE",
            "SESSION_COOKIE_HTTP_ONLY",
            "SESSION_COOKIE_SAME_SITE",
            "SESSION_CLEANUP_INTERVAL_HOURS",
        ] {
            std::env::remove_var(key);
        }

        let config = SessionConfig::from_env();
        assert_eq!(config.timeout, Duration::minutes(30));
        assert_eq!(config.max_age, Duration::hours(24));
        assert_eq!(config.cookie_name, "freshcredit_session");
        assert_eq!(config.cookie_path, "/");
        assert!(config.cookie_secure);
        assert!(config.cookie_http_only);
        assert!(matches!(config.cookie_same_site, SameSitePolicy::Lax));
        assert_eq!(config.cleanup_interval, Duration::hours(1));
    }

    #[test]
    fn test_from_env_custom_values() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("SESSION_TIMEOUT_MINUTES", "60");
        std::env::set_var("SESSION_MAX_AGE_HOURS", "48");
        std::env::set_var("SESSION_COOKIE_NAME", "custom_session");
        std::env::set_var("SESSION_COOKIE_DOMAIN", "example.com");
        std::env::set_var("SESSION_COOKIE_PATH", "/api");
        std::env::set_var("SESSION_COOKIE_SECURE", "false");
        std::env::set_var("SESSION_COOKIE_HTTP_ONLY", "false");
        std::env::set_var("SESSION_COOKIE_SAME_SITE", "Strict");
        std::env::set_var("SESSION_CLEANUP_INTERVAL_HOURS", "2");

        let config = SessionConfig::from_env();
        assert_eq!(config.timeout, Duration::minutes(60));
        assert_eq!(config.max_age, Duration::hours(48));
        assert_eq!(config.cookie_name, "custom_session");
        // TAG: surface=security owner=platform-team rule=MID-001
        assert_eq!(config.cookie_domain, Some("example.com".to_string()));
        assert_eq!(config.cookie_path, "/api");
        assert!(!config.cookie_secure);
        assert!(!config.cookie_http_only);
        assert!(matches!(config.cookie_same_site, SameSitePolicy::Strict));
        assert_eq!(config.cleanup_interval, Duration::hours(2));

        // Clean up
        for key in [
            "SESSION_TIMEOUT_MINUTES",
            "SESSION_MAX_AGE_HOURS",
            "SESSION_COOKIE_NAME",
            "SESSION_COOKIE_DOMAIN",
            "SESSION_COOKIE_PATH",
            "SESSION_COOKIE_SECURE",
            "SESSION_COOKIE_HTTP_ONLY",
            "SESSION_COOKIE_SAME_SITE",
            "SESSION_CLEANUP_INTERVAL_HOURS",
        ] {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn test_from_env_same_site_none() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("SESSION_COOKIE_SAME_SITE", "none");

        let config = SessionConfig::from_env();
        assert!(matches!(config.cookie_same_site, SameSitePolicy::None));

        std::env::remove_var("SESSION_COOKIE_SAME_SITE");
    }

    #[test]
    fn test_from_env_invalid_parsing_fallback() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("SESSION_TIMEOUT_MINUTES", "not_a_number");
        std::env::set_var("SESSION_COOKIE_SECURE", "not_a_bool");

        let config = SessionConfig::from_env();
        assert_eq!(config.timeout, Duration::minutes(30)); // fallback
        assert!(config.cookie_secure); // fallback

        std::env::remove_var("SESSION_TIMEOUT_MINUTES");
        std::env::remove_var("SESSION_COOKIE_SECURE");
    }
}
