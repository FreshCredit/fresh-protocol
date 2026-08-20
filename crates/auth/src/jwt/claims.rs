//! JWT Claims definitions and utilities

use serde::{Deserialize, Serialize};

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Extended JWT claims with additional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
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
    /// JWT ID (unique token identifier)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
    /// Not before (Unix timestamp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nbf: Option<i64>,
    /// Entra ID
    #[serde(rename = "entra_id")]
    pub entra_id: String,
    /// Verified ID credential ID
    #[serde(
        rename = "verified_id_credential_id",
        skip_serializing_if = "Option::is_none"
    )]
    pub verified_id_credential_id: Option<String>,
    /// Roles (custom claim)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub roles: Vec<String>,
    /// Permissions (custom claim)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub permissions: Vec<String>,
}

// TAG: surface=auth owner=security-team rule=RBAC-001
impl Claims {
    /// Create a new claims builder
    pub fn builder(sub: impl Into<String>) -> ClaimsBuilder {
        ClaimsBuilder::new(sub)
    }

    /// Check if the token is expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.exp < now
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    /// Check if the token is valid (not expired and not before current time)
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let now = chrono::Utc::now().timestamp();

        if self.exp < now {
            return false;
        }

        if let Some(nbf) = self.nbf {
            if now < nbf {
                return false;
            }
        }

        true
    }

    /// Get remaining time until expiration in seconds
    #[must_use]
    pub fn time_until_expiry(&self) -> i64 {
        let now = chrono::Utc::now().timestamp();
        (self.exp - now).max(0)
    }

    /// Check if user has a specific role
    #[must_use]
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r.eq_ignore_ascii_case(role))
    }

    /// Check if user has a specific permission
    #[must_use]
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions
            .iter()
            .any(|p| p.eq_ignore_ascii_case(permission))
    }
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Builder for Claims
#[derive(Debug, Default)]
pub struct ClaimsBuilder {
    sub: String,
    email: Option<String>,
    name: Option<String>,
    iat: i64,
    exp: Option<i64>,
    iss: String,
    aud: String,
    jti: Option<String>,
    nbf: Option<i64>,
    entra_id: Option<String>,
    verified_id_credential_id: Option<String>,
    roles: Vec<String>,
    permissions: Vec<String>,
}

impl ClaimsBuilder {
    /// Create a new claims builder
    pub fn new(sub: impl Into<String>) -> Self {
        Self {
            sub: sub.into(),
            iat: chrono::Utc::now().timestamp(),
            iss: "freshcredit".to_string(),
            aud: "freshcredit-api".to_string(),
            ..Default::default()
        }
    }

    /// Set email
    #[must_use]
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    /// Set name
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set expiration time from now (in seconds)
    #[must_use]
    pub const fn expires_in(mut self, seconds: i64) -> Self {
        self.exp = Some(self.iat + seconds);
        self
    }

    /// Set absolute expiration time (Unix timestamp)
    #[must_use]
    pub const fn exp(mut self, exp: i64) -> Self {
        self.exp = Some(exp);
        self
    }

    /// Set issuer
    #[must_use]
    pub fn issuer(mut self, iss: impl Into<String>) -> Self {
        self.iss = iss.into();
        self
    }

    /// Set audience
    #[must_use]
    pub fn audience(mut self, aud: impl Into<String>) -> Self {
        self.aud = aud.into();
        self
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    /// Set JWT ID
    #[must_use]
    pub fn jti(mut self, jti: impl Into<String>) -> Self {
        self.jti = Some(jti.into());
        self
    }

    /// Set not before time (Unix timestamp)
    #[must_use]
    pub const fn nbf(mut self, nbf: i64) -> Self {
        self.nbf = Some(nbf);
        self
    }

    /// Set Entra ID
    #[must_use]
    pub fn entra_id(mut self, entra_id: impl Into<String>) -> Self {
        self.entra_id = Some(entra_id.into());
        self
    }

    /// Set Verified ID credential ID
    #[must_use]
    pub fn verified_id_credential_id(mut self, id: impl Into<String>) -> Self {
        self.verified_id_credential_id = Some(id.into());
        self
    }

    /// Add a role
    #[must_use]
    pub fn role(mut self, role: impl Into<String>) -> Self {
        self.roles.push(role.into());
        self
    }

    /// Add multiple roles
    #[must_use]
    pub fn roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    /// Add a permission
    #[must_use]
    pub fn permission(mut self, permission: impl Into<String>) -> Self {
        self.permissions.push(permission.into());
        self
    }

    /// Add multiple permissions
    #[must_use]
    pub fn permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }

    /// Build the Claims
    #[must_use]
    pub fn build(self) -> Claims {
        Claims {
            sub: self.sub,
            email: self.email.unwrap_or_default(),
            name: self.name.unwrap_or_default(),
            iat: self.iat,
            exp: self.exp.unwrap_or(self.iat + 3600), // Default 1 hour
            iss: self.iss,
            aud: self.aud,
            jti: self.jti,
            nbf: self.nbf,
            entra_id: self.entra_id.unwrap_or_default(),
            verified_id_credential_id: self.verified_id_credential_id,
            // TAG: surface=auth owner=platform-team rule=GENERAL-001
            roles: self.roles,
            permissions: self.permissions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claims_builder() {
        let claims = Claims::builder("user-123")
            .email("test@example.com")
            .name("Test User")
            .expires_in(3600)
            .role("admin")
            .permission("users:read")
            .entra_id("entra-456")
            .build();

        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(claims.name, "Test User");
        assert!(claims.roles.contains(&"admin".to_string()));
        assert!(claims.permissions.contains(&"users:read".to_string()));
        assert_eq!(claims.entra_id, "entra-456");
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    #[test]
    fn test_claims_validation() {
        let now = chrono::Utc::now().timestamp();

        let valid_claims = Claims {
            sub: "user-1".to_string(),
            email: "test@example.com".to_string(),
            name: "Test".to_string(),
            iat: now,
            exp: now + 3600,
            iss: "freshcredit".to_string(),
            aud: "freshcredit-api".to_string(),
            jti: None,
            nbf: None,
            entra_id: "entra-1".to_string(),
            verified_id_credential_id: None,
            roles: vec![],
            permissions: vec![],
        };

        assert!(valid_claims.is_valid());
        assert!(!valid_claims.is_expired());

        let expired_claims = Claims {
            exp: now - 3600,
            ..valid_claims
        };

        assert!(!expired_claims.is_valid());
        assert!(expired_claims.is_expired());
    }

    #[test]
    fn test_role_and_permission_checks() {
        let claims = Claims::builder("user-1")
            .role("admin")
            .role("user")
            .permission("users:read")
            .permission("users:write")
            .build();

        // TAG: surface=auth owner=security-team rule=RBAC-001
        assert!(claims.has_role("admin"));
        assert!(claims.has_role("ADMIN")); // Case insensitive
        assert!(!claims.has_role("superadmin"));

        assert!(claims.has_permission("users:read"));
        assert!(!claims.has_permission("posts:delete"));
    }

    #[test]
    fn test_claims_nbf_validation() {
        let now = chrono::Utc::now().timestamp();

        // Token with nbf in the future should not be valid
        let future_nbf = Claims {
            sub: "user-1".to_string(),
            email: "test@example.com".to_string(),
            name: "Test".to_string(),
            iat: now,
            exp: now + 3600,
            iss: "freshcredit".to_string(),
            aud: "freshcredit-api".to_string(),
            jti: None,
            nbf: Some(now + 300), // Not valid for 5 minutes
            entra_id: "entra-1".to_string(),
            verified_id_credential_id: None,
            roles: vec![],
            permissions: vec![],
        };

        // TAG: surface=auth owner=security-team rule=RBAC-001
        assert!(!future_nbf.is_valid());
        assert!(!future_nbf.is_expired()); // Not expired, just not yet valid

        // Token with nbf in the past should be valid
        let past_nbf = Claims {
            nbf: Some(now - 300),
            ..future_nbf
        };
        assert!(past_nbf.is_valid());
    }

    #[test]
    fn test_time_until_expiry() {
        let now = chrono::Utc::now().timestamp();

        let claims = Claims {
            sub: "user-1".to_string(),
            email: "test@example.com".to_string(),
            name: "Test".to_string(),
            iat: now,
            exp: now + 3600,
            iss: "freshcredit".to_string(),
            aud: "freshcredit-api".to_string(),
            jti: None,
            nbf: None,
            entra_id: "entra-1".to_string(),
            verified_id_credential_id: None,
            roles: vec![],
            permissions: vec![],
        };

        let remaining = claims.time_until_expiry();
        // Should be approximately 3600 seconds (allow 1s for test execution)
        assert!((3599..=3600).contains(&remaining));

        let expired = Claims {
            exp: now - 100,
            ..claims
        };
        assert_eq!(expired.time_until_expiry(), 0);
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    #[test]
    fn test_builder_defaults() {
        let claims = Claims::builder("user-123").build();

        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.email, "");
        assert_eq!(claims.name, "");
        assert_eq!(claims.entra_id, "");
        assert_eq!(claims.iss, "freshcredit");
        assert_eq!(claims.aud, "freshcredit-api");
        assert!(claims.roles.is_empty());
        assert!(claims.permissions.is_empty());
        assert!(claims.jti.is_none());
        assert!(claims.nbf.is_none());
        // Default expiry is iat + 3600
        assert!(claims.exp >= claims.iat + 3600 - 1 && claims.exp <= claims.iat + 3600 + 1);
    }

    #[test]
    fn test_builder_chaining() {
        let claims = Claims::builder("user-456")
            .email("alice@example.com")
            .name("Alice")
            .issuer("custom-issuer")
            .audience("custom-aud")
            .jti("unique-id-123")
            .nbf(1_234_567_890)
            .entra_id("entra-789")
            .verified_id_credential_id("cred-abc")
            .role("admin")
            .permission("all")
            .build();

        // TAG: surface=auth owner=security-team rule=RBAC-001
        assert_eq!(claims.sub, "user-456");
        assert_eq!(claims.email, "alice@example.com");
        assert_eq!(claims.name, "Alice");
        assert_eq!(claims.iss, "custom-issuer");
        assert_eq!(claims.aud, "custom-aud");
        assert_eq!(claims.jti, Some("unique-id-123".to_string()));
        assert_eq!(claims.nbf, Some(1_234_567_890));
        assert_eq!(claims.entra_id, "entra-789");
        assert_eq!(
            claims.verified_id_credential_id,
            Some("cred-abc".to_string())
        );
        assert_eq!(claims.roles, vec!["admin"]);
        assert_eq!(claims.permissions, vec!["all"]);
    }

    #[test]
    fn test_builder_exp_overrides_expires_in() {
        let now = chrono::Utc::now().timestamp();

        // When both exp and expires_in are set, the last one wins
        let claims = Claims::builder("user-1")
            .expires_in(3600)
            .exp(now + 7200)
            .build();

        assert_eq!(claims.exp, now + 7200);
    }

    #[test]
    fn test_empty_roles_and_permissions() {
        let claims = Claims::builder("user-1").build();
        assert!(!claims.has_role("admin"));
        assert!(!claims.has_permission("read"));
    }
}
