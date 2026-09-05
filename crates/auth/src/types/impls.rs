use super::types::*;
use std::fmt;

// TAG: surface=auth owner=security-team rule=RBAC-001
impl AuthenticatedUser {
    /// Create a new `AuthenticatedUser` with required fields
    pub fn new(
        user_id: impl Into<String>,
        email: impl Into<String>,
        name: impl Into<String>,
        entra_id: impl Into<String>,
    ) -> Self {
        Self {
            user_id: user_id.into(),
            email: email.into(),
            name: name.into(),
            given_name: None,
            family_name: None,
            mobile_phone: None,
            job_title: None,
            street_address: None,
            city: None,
            state: None,
            postal_code: None,
            country: None,
            entra_id: entra_id.into(),
            verified_id_credential_id: None,
            roles: Vec::new(),
        }
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    /// Get the user's full name, falling back to email if not set
    #[must_use]
    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.email
        } else {
            &self.name
        }
    }

    /// Check if the user has a complete profile
    #[must_use]
    pub const fn has_complete_profile(&self) -> bool {
        self.given_name.is_some() && self.family_name.is_some() && self.mobile_phone.is_some()
    }

    /// Check if the user has a specific role (case-insensitive)
    #[must_use]
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r.eq_ignore_ascii_case(role))
    }

    /// Check if the user is an admin (has 'admin' or 'internal' role)
    #[must_use]
    pub fn is_admin(&self) -> bool {
        self.has_role("admin") || self.has_role("internal")
    }
}

// TAG: surface=auth owner=security-team rule=RBAC-001
impl fmt::Display for AuthenticatedUser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} <{}>", self.display_name(), self.email)
    }
}

impl AuthError {
    /// Convert to HTTP status code
    #[must_use]
    pub const fn status_code(&self) -> http::StatusCode {
        use http::StatusCode;
        match self {
            Self::MissingAuthorization
            | Self::TokenExpired
            | Self::InvalidSignature
            | Self::UserNotFound(_) => StatusCode::UNAUTHORIZED,
            Self::InvalidTokenFormat => StatusCode::BAD_REQUEST,
            Self::InsufficientPermissions(_) => StatusCode::FORBIDDEN,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JwtBearer => write!(f, "jwt_bearer"),
            Self::CookieSession => write!(f, "cookie_session"),
            Self::Oauth => write!(f, "oauth"),
            Self::ApiKey => write!(f, "api_key"),
            Self::WebAuthn => write!(f, "webauthn"),
        }
    }
}

// TAG: surface=auth owner=security-team rule=RBAC-001
impl JwtClaims {
    /// Create new claims for a user
    pub fn new(
        user_id: impl Into<String>,
        email: impl Into<String>,
        name: impl Into<String>,
        entra_id: impl Into<String>,
        expires_in_secs: i64,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            sub: user_id.into(),
            email: email.into(),
            name: name.into(),
            iat: now,
            exp: now + expires_in_secs,
            iss: "freshcredit".to_string(),
            aud: "freshcredit-api".to_string(),
            entra_id: entra_id.into(),
            verified_id_credential_id: None,
            roles: Vec::new(),
        }
    }

    /// Convert to `AuthenticatedUser`
    #[must_use]
    pub fn to_authenticated_user(&self) -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: self.sub.clone(),
            email: self.email.clone(),
            name: self.name.clone(),
            given_name: None,
            family_name: None,
            mobile_phone: None,
            job_title: None,
            street_address: None,
            city: None,
            state: None,
            postal_code: None,
            country: None,
            entra_id: self.entra_id.clone(),
            verified_id_credential_id: self.verified_id_credential_id.clone(),
            roles: self.roles.clone(),
        }
    }
}

// TAG: surface=auth owner=security-team rule=RBAC-001
impl fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unverified => write!(f, "unverified"),
            Self::Pending => write!(f, "pending"),
            Self::Verified => write!(f, "verified"),
            Self::Expired => write!(f, "expired"),
            Self::Revoked => write!(f, "revoked"),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        // P0-SECURITY: JWT_SECRET must be set explicitly in ALL builds.
        // In test builds, fall back to an invalid placeholder that
        // validate_for_production() will reject to prevent accidental use.
        match Self::from_env() {
            Ok(cfg) => cfg,
            Err(_) if cfg!(test) => Self {
                jwt_secret: "INVALID-DEV-SECRET-SET-JWT_SECRET-ENV-VAR".to_string(),
                token_expiration_secs: 3600,
                refresh_token_expiration_secs: 86400 * 7,
                oauth_client_id: None,
                oauth_client_secret: None,
                oauth_tenant_id: None,
                allowed_redirect_urls: vec![],
            },
            Err(e) => panic!(
                "JWT_SECRET environment variable must be set - do not use placeholder in any build: {e}"
            ),
        }
    }
}

// TAG: surface=auth owner=security-team rule=RBAC-001
impl AuthConfig {
    /// Create a new `AuthConfig` from environment variables.
    /// Returns an error if `JWT_SECRET` is not set in production.
    ///
    /// # Security
    /// Use this method in production instead of `Default::default()`
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn from_env() -> Result<Self, AuthConfigError> {
        let jwt_secret =
            std::env::var("JWT_SECRET").map_err(|_| AuthConfigError::MissingJwtSecret)?;

        if jwt_secret.len() < 32 {
            return Err(AuthConfigError::WeakJwtSecret);
        }

        Ok(Self {
            jwt_secret,
            token_expiration_secs: 3600,
            refresh_token_expiration_secs: 86400 * 7,
            oauth_client_id: std::env::var("OAUTH_CLIENT_ID").ok(),
            oauth_client_secret: std::env::var("OAUTH_CLIENT_SECRET").ok(),
            oauth_tenant_id: std::env::var("OAUTH_TENANT_ID").ok(),
            allowed_redirect_urls: std::env::var("ALLOWED_REDIRECT_URLS")
                .map(|s| s.split(',').map(String::from).collect())
                .unwrap_or_default(),
        })
    }

    /// Validate that this config is secure for production use
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn validate_for_production(&self) -> Result<(), AuthConfigError> {
        if self.jwt_secret == "INVALID-DEV-SECRET-SET-JWT_SECRET-ENV-VAR" {
            return Err(AuthConfigError::MissingJwtSecret);
        }
        if self.jwt_secret.len() < 32 {
            return Err(AuthConfigError::WeakJwtSecret);
        }
        Ok(())
    }
}
