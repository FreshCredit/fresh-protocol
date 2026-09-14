use serde::{Deserialize, Serialize};
use thiserror::Error;

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Authenticated user information extracted from JWT claims
///
/// This struct is the unified representation of an authenticated user
/// across all `FreshCredit` services (API, web, scheduler, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthenticatedUser {
    /// Unique user identifier (UUID)
    pub user_id: String,
    /// User's email address
    pub email: String,
    /// Display name
    pub name: String,
    /// First/given name
    pub given_name: Option<String>,
    /// Last/family name
    pub family_name: Option<String>,
    /// Mobile phone number
    pub mobile_phone: Option<String>,
    /// Job title
    pub job_title: Option<String>,
    /// Street address
    pub street_address: Option<String>,
    /// City
    pub city: Option<String>,
    /// State/province
    pub state: Option<String>,
    /// Postal/ZIP code
    pub postal_code: Option<String>,
    /// Country
    pub country: Option<String>,
    /// Microsoft Entra ID (OID)
    pub entra_id: String,
    /// Verified ID credential ID (if applicable)
    pub verified_id_credential_id: Option<String>,
    /// User roles from JWT claims
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub roles: Vec<String>,
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Authentication errors
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// Missing or invalid authorization header
    #[error("Missing or invalid authorization header")]
    MissingAuthorization,

    /// Invalid token format
    #[error("Invalid token format")]
    InvalidTokenFormat,

    /// Token has expired
    #[error("Token has expired")]
    TokenExpired,

    /// Invalid token signature
    #[error("Invalid token signature")]
    InvalidSignature,

    /// User not found
    #[error("User not found: {0}")]
    UserNotFound(String),

    /// Insufficient permissions
    #[error("Insufficient permissions: {0}")]
    InsufficientPermissions(String),

    /// Internal error
    #[error("Internal authentication error: {0}")]
    Internal(String),
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Configuration errors for `AuthConfig`
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AuthConfigError {
    /// `JWT_SECRET` environment variable not set
    #[error("JWT_SECRET environment variable must be set")]
    MissingJwtSecret,

    /// JWT secret is too weak (less than 32 characters)
    #[error("JWT_SECRET must be at least 32 characters long")]
    WeakJwtSecret,
}

/// Authentication method used
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    /// JWT Bearer token
    JwtBearer,
    /// Cookie-based session
    CookieSession,
    /// OAuth 2.0 / OIDC
    Oauth,
    /// API key
    ApiKey,
    /// `WebAuthn` / biometric
    WebAuthn,
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Token claims for JWT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (user ID)
    pub sub: String,
    /// Email
    pub email: String,
    /// Name
    pub name: String,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Expiration (Unix timestamp)
    pub exp: i64,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: String,
    /// Entra ID
    #[serde(rename = "entra_id")]
    pub entra_id: String,
    /// Verified ID credential ID (optional)
    #[serde(
        rename = "verified_id_credential_id",
        skip_serializing_if = "Option::is_none"
    )]
    pub verified_id_credential_id: Option<String>,
    /// User roles
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub roles: Vec<String>,
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// SSO/OAuth authentication context
#[derive(Debug, Clone)]
pub struct SsoContext {
    /// Identity provider (e.g., "microsoft", "google")
    pub provider: String,
    /// Provider-specific user ID
    pub provider_user_id: String,
    /// Access token from provider
    pub access_token: String,
    /// Refresh token (if available)
    pub refresh_token: Option<String>,
    /// Token expiration
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Verification status for Verified ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Not verified
    Unverified,
    /// Verification pending
    Pending,
    /// Verified
    Verified,
    /// Verification expired
    Expired,
    /// Verification revoked
    Revoked,
}

/// Authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// JWT secret key
    pub jwt_secret: String,
    /// Token expiration in seconds
    pub token_expiration_secs: i64,
    /// Refresh token expiration in seconds
    pub refresh_token_expiration_secs: i64,
    /// OAuth client ID (for Entra ID)
    pub oauth_client_id: Option<String>,
    /// OAuth client secret
    pub oauth_client_secret: Option<String>,
    /// OAuth tenant ID
    pub oauth_tenant_id: Option<String>,
    /// Allowed redirect URLs for OAuth
    pub allowed_redirect_urls: Vec<String>,
}
