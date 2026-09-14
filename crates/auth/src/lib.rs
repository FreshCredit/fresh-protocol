//! `FreshCredit` Authentication Crate
//!
//! This crate provides shared authentication functionality for the `FreshCredit` platform,
//! including JWT management, OAuth/OIDC integration, session handling, and RBAC.

// TAG: surface=auth owner=security-team rule=RBAC-001
#![allow(clippy::wildcard_imports)]
#![deny(missing_docs)]
#![deny(unsafe_code)]
//!
//! # Features
//!
//! - `jwt` - JWT token generation and validation (enabled by default)
//! - `oauth` - OAuth 2.0 / OIDC client support
//! - `rbac` - Role-based access control
//! - `session` - Session management integration
//! - `biometric` - WebAuthn/biometric authentication
//! - `axum` - Axum framework integration (extractors, middleware)
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use auth::{AuthenticatedUser, jwt::JwtManager};
//!
//! // Create a JWT manager
//! let jwt_manager = JwtManager::new("your-secret-key");
//!
//! // Generate a token
//! let token = jwt_manager.generate_token(
//!     "user-123",
//!     "user@example.com",
//!     "John Doe",
//!     "entra-id-456",
//!     3600, // expires in 1 hour
//! ).unwrap();
//!
//! // Validate the token
//! let claims = jwt_manager.validate_token(&token).unwrap();
//! ```

#![warn(missing_docs)]

// TAG: surface=auth owner=security-team rule=RBAC-001
// Re-export core types
pub mod types;
pub use types::{
    AuthConfig, AuthError, AuthMethod, AuthenticatedUser, JwtClaims, SsoContext, VerificationStatus,
};

// JWT module (requires "jwt" feature)
#[cfg(feature = "jwt")]
pub mod jwt;
#[cfg(feature = "jwt")]
pub use jwt::JwtManager;

// OAuth module (requires "oauth" feature)
#[cfg(feature = "oauth")]
pub mod oauth;

// RBAC module (requires "rbac" feature)
#[cfg(feature = "rbac")]
pub mod rbac;

// Session module (requires "session" feature)
#[cfg(feature = "session")]
pub mod session;

// Axum extractors (requires "axum" feature)
#[cfg(feature = "axum")]
pub mod extractors;
#[cfg(feature = "axum")]
pub use extractors::{AuthExtractor, OptionalAuthExtractor};

// Middleware module
#[cfg(feature = "axum")]
pub mod middleware;

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default token expiration time in seconds (1 hour)
pub const DEFAULT_TOKEN_EXPIRATION_SECS: i64 = 3600;

/// Default refresh token expiration time in seconds (7 days)
pub const DEFAULT_REFRESH_TOKEN_EXPIRATION_SECS: i64 = 86400 * 7;

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Create a new authenticated user (convenience function)
pub fn user(
    user_id: impl Into<String>,
    // TAG: surface=auth owner=security-team rule=RBAC-001
    email: impl Into<String>,
    name: impl Into<String>,
    entra_id: impl Into<String>,
) -> AuthenticatedUser {
    AuthenticatedUser::new(user_id, email, name, entra_id)
}

/// Parse a bearer token from an Authorization header
/// # Errors
///
/// Returns an error if the operation fails.
pub fn parse_bearer_token(header: &str) -> Result<&str, AuthError> {
    #[cfg(feature = "jwt")]
    {
        JwtManager::extract_bearer_token(header)
    }
    #[cfg(not(feature = "jwt"))]
    {
        header
            .strip_prefix("Bearer ")
            .map(|t| t.trim())
            .ok_or(AuthError::InvalidTokenFormat)
    }
}

#[cfg(test)]
// TAG: surface=auth owner=platform-team rule=GENERAL-001
mod tests {
    use super::*;

    #[test]
    fn test_user_convenience_function() {
        let user = user("123", "test@example.com", "Test User", "entra-456");
        assert_eq!(user.user_id, "123");
        assert_eq!(user.email, "test@example.com");
    }

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_TOKEN_EXPIRATION_SECS, 3600);
        assert_eq!(DEFAULT_REFRESH_TOKEN_EXPIRATION_SECS, 604_800);
    }

    #[test]
    fn test_version() {
        // VERSION is a const defined at compile time, this test ensures it's not empty
        // The const_is_empty lint fires because VERSION is a const, but this is intentional
        #[allow(clippy::const_is_empty)]
        {
            assert!(!VERSION.is_empty());
        }
    }
}
