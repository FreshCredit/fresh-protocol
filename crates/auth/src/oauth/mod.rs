//! OAuth 2.0 / OIDC integration module (RFC 9700 / BCP 240 hardened)
//!
//! This module provides OAuth 2.0 and `OpenID` Connect client functionality.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};

fn url_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Compute RFC 7636 / RFC 9700 S256 PKCE `code_challenge` from `code_verifier`.
#[must_use]
pub fn pkce_s256_challenge(code_verifier: &str) -> String {
    let digest = Sha256::digest(code_verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// OAuth client configuration (`client_secret` redacted in `Debug` output)
#[derive(Clone)]
pub struct OAuthConfig {
    /// Client ID
    pub client_id: String,
    /// Client secret
    pub client_secret: String,
    /// Authorization endpoint URL
    pub auth_url: String,
    /// Token endpoint URL
    pub token_url: String,
    /// Redirect URI
    pub redirect_uri: String,
}

impl std::fmt::Debug for OAuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("auth_url", &self.auth_url)
            .field("token_url", &self.token_url)
            .field("redirect_uri", &self.redirect_uri)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_config_creation() {
        let config = OAuthConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-secret".to_string(),
            auth_url: "https://auth.example.com/authorize".to_string(),
            token_url: "https://auth.example.com/token".to_string(),
            redirect_uri: "https://app.example.com/callback".to_string(),
        };

        assert_eq!(config.client_id, "test-client-id");
        assert_eq!(config.client_secret, "test-secret");
        assert_eq!(config.auth_url, "https://auth.example.com/authorize");
        assert_eq!(config.token_url, "https://auth.example.com/token");
        assert_eq!(config.redirect_uri, "https://app.example.com/callback");
    }

    #[test]
    fn test_oauth_config_clone() {
        let config = OAuthConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-secret".to_string(),
            auth_url: "https://auth.example.com/authorize".to_string(),
            token_url: "https://auth.example.com/token".to_string(),
            redirect_uri: "https://app.example.com/callback".to_string(),
        };

        // TAG: surface=auth owner=security-team rule=RBAC-001
        let cloned = config.clone();
        assert_eq!(cloned.client_id, config.client_id);
        assert_eq!(cloned.client_secret, config.client_secret);
    }

    #[test]
    fn test_oauth_config_debug_redacts_secret() {
        let config = OAuthConfig {
            client_id: "id".to_string(),
            client_secret: "super-secret-value".to_string(),
            auth_url: "https://auth".to_string(),
            token_url: "https://token".to_string(),
            redirect_uri: "https://callback".to_string(),
        };

        let debug_str = format!("{config:?}");
        assert!(debug_str.contains("OAuthConfig"));
        assert!(debug_str.contains("id"));
        assert!(!debug_str.contains("super-secret-value"));
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn test_build_authorization_url_enforces_pkce_state_and_encoding() {
        let client = OAuthClient::new(OAuthConfig {
            client_id: "cid-1".to_string(),
            client_secret: "sec".to_string(),
            auth_url: "https://auth.example.com/authorize".to_string(),
            token_url: "https://auth.example.com/token".to_string(),
            redirect_uri: "https://app.example.com/cb?next=/home&x=1".to_string(),
        });
        let challenge = pkce_s256_challenge("verifier-with-high-entropy-1234567890");
        let url = client.build_authorization_url("csrf-state-xyz", &challenge);
        assert!(url.contains("state=csrf-state-xyz"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains(&format!("code_challenge={challenge}")));
        assert!(url
            .contains("redirect_uri=https%3A%2F%2Fapp.example.com%2Fcb%3Fnext%3D%2Fhome%26x%3D1"));
    }
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// OAuth client for authentication flows
#[derive(Debug, Clone)]
pub struct OAuthClient {
    config: OAuthConfig,
}

impl OAuthClient {
    /// Create a new OAuth client
    #[must_use]
    pub const fn new(config: OAuthConfig) -> Self {
        Self { config }
    }

    /// Build an RFC 9700 / BCP 240 compliant authorization URL with explicit CSRF `state`,
    /// PKCE `S256` challenge, and percent-encoded `redirect_uri`.
    #[must_use]
    pub fn build_authorization_url(&self, state: &str, code_challenge_s256: &str) -> String {
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&state={}&code_challenge={}&code_challenge_method=S256",
            self.config.auth_url,
            url_encode_component(&self.config.client_id),
            url_encode_component(&self.config.redirect_uri),
            url_encode_component(state),
            url_encode_component(code_challenge_s256),
        )
    }

    /// Legacy authorization URL helper (prefers [`Self::build_authorization_url`] for PKCE/CSRF).
    #[must_use]
    pub fn get_auth_url(&self) -> String {
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code",
            self.config.auth_url,
            url_encode_component(&self.config.client_id),
            url_encode_component(&self.config.redirect_uri),
        )
    }
}

/// Token response from OAuth provider (`access_token` / `refresh_token` redacted in `Debug`)
#[derive(Clone)]
pub struct TokenResponse {
    /// Access token
    pub access_token: String,
    /// Token type (e.g., "Bearer")
    pub token_type: String,
    /// Expiration time in seconds
    pub expires_in: Option<i64>,
    /// Refresh token
    pub refresh_token: Option<String>,
}

impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field("access_token", &"[REDACTED]")
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}
