//! Configuration constants and authentication structures.

use serde::{Deserialize, Serialize};

// ============================================================================
// Security Constants
// ============================================================================

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Maximum request body size (1MB)
pub const MAX_REQUEST_BODY_SIZE: usize = 1024 * 1024;

/// Rate limit: requests per window
pub const RATE_LIMIT_REQUESTS: u64 = 100;

/// Rate limit: window duration in seconds
#[allow(dead_code)]
pub const RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Default retry delay in seconds
pub const DEFAULT_RETRY_DELAY_SECS: u64 = 5;

/// Maximum retry delay in seconds
pub const MAX_RETRY_DELAY_SECS: u64 = 60;

/// Health check interval in seconds
pub const HEALTH_CHECK_INTERVAL_SECS: u64 = 30;

/// Number of recent blocks to fetch for display
#[allow(dead_code)]
pub const RECENT_BLOCKS_COUNT: usize = 5;

/// Number of blocks for partial display
pub const BLOCKS_PARTIAL_COUNT: usize = 3;

/// Number of blocks to fetch for recent blocks API
pub const API_RECENT_BLOCKS_COUNT: u32 = 5;

/// CORS cache max age in seconds (1 hour)
pub const CORS_MAX_AGE_SECS: u32 = 3600;

// ============================================================================
// Authentication & Authorization
// ============================================================================

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// JWT secret from environment (loaded once at startup)
/// SECURITY: Panics in production if not set to prevent using default secrets
pub static JWT_SECRET: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| match std::env::var("JWT_SECRET") {
        Ok(secret) if !secret.is_empty() => secret,
        _ => {
            panic!("JWT_SECRET must be set in environment");
        }
    });

/// API key for service-to-service authentication
/// SECURITY: Panics in production if not set to prevent using default secrets
pub static API_KEY: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| match std::env::var("API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            panic!("API_KEY must be set in environment");
        }
    });

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// JWT Claims for authenticated requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (`user_id` or `service_id`)
    pub sub: String,
    /// Issued at timestamp
    pub iat: i64,
    /// Expiration timestamp
    pub exp: i64,
    /// Token type: "user" or "service"
    pub token_type: String,
}

#[allow(dead_code)]
impl Claims {
    /// Create new claims for a user
    pub fn new_user(user_id: &str, expiry_hours: i64) -> Self {
        let now = chrono::Utc::now();
        Self {
            sub: user_id.to_string(),
            iat: now.timestamp(),
            exp: (now + chrono::Duration::hours(expiry_hours)).timestamp(),
            token_type: "user".to_string(),
        }
    }

    /// Create new claims for a service account
    pub fn new_service(service_id: &str, expiry_hours: i64) -> Self {
        let now = chrono::Utc::now();
        Self {
            sub: service_id.to_string(),
            iat: now.timestamp(),
            exp: (now + chrono::Duration::hours(expiry_hours)).timestamp(),
            token_type: "service".to_string(),
        }
    }
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Authentication error response
#[derive(Serialize)]
pub struct AuthError {
    pub error: String,
    pub message: String,
}

// ============================================================================
// NOMT Verification
// ============================================================================

/// NOMT verification result
#[derive(Debug, Deserialize)]
pub struct NomtVerifyResult {
    pub root: String,
    pub proof_available: bool,
    pub timestamp: Option<String>,
}

// ============================================================================
// Anchor Request Validation
// ============================================================================

/// Maximum length for `user_id` field
pub const MAX_USER_ID_LENGTH: usize = 64;
/// Maximum length for `report_type` field
pub const MAX_REPORT_TYPE_LENGTH: usize = 32;
/// Maximum length for `report_data` field (100KB)
pub const MAX_REPORT_DATA_LENGTH: usize = 100 * 1024;

/// Valid report types for validation
pub const VALID_REPORT_TYPES: &[&str] = &[
    "plaid_financial",
    "identity_verification",
    "transaction_history",
    "balance_snapshot",
    "consumer_preferences",
    "provider_requirements",
    "financial",
    "professional",
    "personal",
    "combined",
    "unified",
    "plaid_data_approval",
    "finalize",
    "plaid_report",
    "consumer_report",
    "demo_report",
];

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[derive(Deserialize)]
pub struct AnchorRequest {
    pub user_id: String,
    pub report_type: String,
    pub report_data: String,
}

impl AnchorRequest {
    /// Validate the request fields
    pub fn validate(&self) -> Result<(), String> {
        // Validate user_id length
        if self.user_id.is_empty() {
            return Err("user_id cannot be empty".to_string());
        }
        if self.user_id.len() > MAX_USER_ID_LENGTH {
            return Err(format!(
                "user_id exceeds maximum length of {MAX_USER_ID_LENGTH} characters"
            ));
        }

        // Validate report_type
        if self.report_type.is_empty() {
            return Err("report_type cannot be empty".to_string());
        }
        if self.report_type.len() > MAX_REPORT_TYPE_LENGTH {
            return Err(format!(
                "report_type exceeds maximum length of {MAX_REPORT_TYPE_LENGTH} characters"
            ));
        }
        // Check if report_type is valid (case-insensitive)
        let report_type_lower = self.report_type.to_lowercase();
        if !VALID_REPORT_TYPES.iter().any(|&t| t == report_type_lower) {
            return Err(format!(
                "Invalid report_type: '{}'. Valid types are: {:?}",
                self.report_type, VALID_REPORT_TYPES
            ));
        }

        // Validate report_data length
        if self.report_data.len() > MAX_REPORT_DATA_LENGTH {
            return Err(format!(
                "report_data exceeds maximum size of {MAX_REPORT_DATA_LENGTH} bytes"
            ));
        }

        // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anchor_request_validate_empty_user_id() {
        let req = AnchorRequest {
            user_id: String::new(),
            report_type: "plaid_financial".to_string(),
            report_data: "data".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_anchor_request_validate_long_user_id() {
        let req = AnchorRequest {
            user_id: "a".repeat(MAX_USER_ID_LENGTH + 1),
            report_type: "plaid_financial".to_string(),
            report_data: "data".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_anchor_request_validate_empty_report_type() {
        let req = AnchorRequest {
            user_id: "user-123".to_string(),
            report_type: String::new(),
            report_data: "data".to_string(),
        };
        assert!(req.validate().is_err());
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    #[test]
    fn test_anchor_request_validate_long_report_type() {
        let req = AnchorRequest {
            user_id: "user-123".to_string(),
            report_type: "a".repeat(MAX_REPORT_TYPE_LENGTH + 1),
            report_data: "data".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_anchor_request_validate_invalid_report_type() {
        let req = AnchorRequest {
            user_id: "user-123".to_string(),
            report_type: "invalid_type".to_string(),
            report_data: "data".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_anchor_request_validate_case_insensitive_report_type() {
        let req = AnchorRequest {
            user_id: "user-123".to_string(),
            report_type: "Plaid_Financial".to_string(),
            report_data: "data".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_anchor_request_validate_valid() {
        let req = AnchorRequest {
            user_id: "user-123".to_string(),
            report_type: "plaid_financial".to_string(),
            report_data: "data".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    #[test]
    fn test_anchor_request_validate_large_report_data() {
        let req = AnchorRequest {
            user_id: "user-123".to_string(),
            report_type: "plaid_financial".to_string(),
            report_data: "x".repeat(MAX_REPORT_DATA_LENGTH + 1),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_claims_new_user() {
        let claims = Claims::new_user("user-123", 24);
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.token_type, "user");
        assert!(claims.iat > 0);
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_claims_new_service() {
        let claims = Claims::new_service("service-abc", 1);
        assert_eq!(claims.sub, "service-abc");
        assert_eq!(claims.token_type, "service");
        assert!(claims.iat > 0);
        assert!(claims.exp > claims.iat);
    }
}
