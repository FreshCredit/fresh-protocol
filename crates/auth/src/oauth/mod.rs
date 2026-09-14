//! OAuth 2.0 / OIDC integration module
//!
//! This module provides OAuth 2.0 and `OpenID` Connect client functionality.

// TAG: surface=auth owner=security-team rule=RBAC-001
/// OAuth client configuration
#[derive(Debug, Clone)]
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
    fn test_oauth_config_debug() {
        let config = OAuthConfig {
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
            auth_url: "https://auth".to_string(),
            token_url: "https://token".to_string(),
            redirect_uri: "https://callback".to_string(),
        };

        let debug_str = format!("{config:?}");
        assert!(debug_str.contains("OAuthConfig"));
        assert!(debug_str.contains("id"));
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

    /// Get the authorization URL
    #[must_use]
    pub fn get_auth_url(&self) -> String {
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code",
            self.config.auth_url, self.config.client_id, self.config.redirect_uri
        )
    }
}

/// Token response from OAuth provider
#[derive(Debug, Clone)]
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
