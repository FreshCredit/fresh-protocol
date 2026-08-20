// TAG: surface=security owner=security-team rule=SEC-001
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};

use super::{Session, SessionArtifact};

/// Session storage trait
#[async_trait]
pub trait SessionStorage: Send + Sync {
    /// Create a new session
    async fn create(&self, session: Session) -> Result<(), SessionError>;

    /// Get a session by ID
    async fn get(&self, session_id: &str) -> Result<Option<Session>, SessionError>;

    /// P0-FIX: Atomically get and refresh session activity to prevent TOCTOU race condition
    async fn get_and_refresh(
        &self,
        session_id: &str,
        timeout: Duration,
    ) -> Result<Option<Session>, SessionError>;

    /// Update session activity timestamp
    async fn update_activity(
        &self,
        session_id: &str,
        timeout: Duration,
    ) -> Result<(), SessionError>;

    /// Delete a session
    async fn delete(&self, session_id: &str) -> Result<(), SessionError>;

    /// Cleanup expired sessions
    async fn cleanup_expired(&self) -> Result<u64, SessionError>;

    /// Get all sessions for a user
    async fn get_user_sessions(&self, user_id: &str) -> Result<Vec<Session>, SessionError>;

    // TAG: surface=security owner=platform-team rule=MID-001
    /// Record a session artifact
    async fn record_artifact(&self, artifact: SessionArtifact) -> Result<(), SessionError>;

    /// Get artifacts for a session
    async fn get_session_artifacts(
        &self,
        session_id: &str,
    ) -> Result<Vec<SessionArtifact>, SessionError>;

    /// Get all artifacts for a user
    async fn get_user_artifacts(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<SessionArtifact>, SessionError>;

    /// Store refresh token for a session
    /// SECURITY: Token should be encrypted before storage
    async fn store_refresh_token(
        &self,
        session_id: &str,
        refresh_token: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), SessionError>;

    /// Get refresh token for a session
    /// Returns the encrypted refresh token if available
    async fn get_refresh_token(&self, session_id: &str) -> Result<Option<String>, SessionError>;

    /// Update tokens after a successful refresh
    async fn update_tokens(
        &self,
        session_id: &str,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: i64,
    ) -> Result<(), SessionError>;
    // TAG: surface=security owner=platform-team rule=GENERAL-001

    /// Get session by access token
    async fn get_by_access_token(
        &self,
        access_token: &str,
    ) -> Result<Option<Session>, SessionError>;

    /// Record a refresh token event for audit
    #[allow(clippy::too_many_arguments)]
    async fn record_refresh_event(
        &self,
        session_id: &str,
        user_id: &str,
        event_type: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        success: bool,
        error_message: Option<&str>,
    ) -> Result<(), SessionError>;

    /// Check rate limit for refresh attempts
    async fn check_refresh_rate_limit(
        &self,
        session_id: &str,
        ip_address: &str,
    ) -> Result<bool, SessionError>;

    /// Increment rate limit counter for refresh attempts
    async fn increment_refresh_rate_limit(
        &self,
        session_id: &str,
        ip_address: &str,
    ) -> Result<(), SessionError>;
}

// TAG: surface=security owner=platform-team rule=MID-001
/// Session-related errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum SessionError {
    /// Session not found in storage.
    #[error("Session not found")]
    NotFound,

    /// Session has expired.
    #[error("Session expired")]
    Expired,

    /// Session has exceeded its maximum age.
    #[error("Session max age exceeded")]
    MaxAgeExceeded,

    /// Database error with details.
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// Serialization/deserialization error with details.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Invalid session data with details.
    #[error("Invalid session data: {0}")]
    InvalidData(String),

    /// Refresh token has expired.
    #[error("Refresh token expired")]
    RefreshTokenExpired,

    /// Refresh token reuse detected (possible attack).
    #[error("Refresh token reuse detected")]
    RefreshTokenReuse,

    /// Rate limit exceeded for token refresh.
    #[error("Rate limit exceeded for token refresh")]
    RateLimitExceeded,

    // TAG: surface=security owner=security-team rule=SEC-001
    /// Encryption/decryption error with details.
    #[error("Encryption error: {0}")]
    EncryptionError(String),
}

impl From<libsql::Error> for SessionError {
    fn from(err: libsql::Error) -> Self {
        Self::DatabaseError(err.to_string())
    }
}

impl From<serde_json::Error> for SessionError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerializationError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_error_display() {
        assert_eq!(SessionError::NotFound.to_string(), "Session not found");
        assert_eq!(SessionError::Expired.to_string(), "Session expired");
        assert_eq!(
            SessionError::MaxAgeExceeded.to_string(),
            "Session max age exceeded"
        );
        assert_eq!(
            SessionError::DatabaseError("conn failed".to_string()).to_string(),
            "Database error: conn failed"
        );
        assert_eq!(
            SessionError::SerializationError("bad json".to_string()).to_string(),
            "Serialization error: bad json" // TAG: surface=security owner=platform-team rule=MID-001
        );
        assert_eq!(
            SessionError::InvalidData("missing field".to_string()).to_string(),
            "Invalid session data: missing field"
        );
        assert_eq!(
            SessionError::RefreshTokenExpired.to_string(),
            "Refresh token expired" // TAG: surface=api
                                    // TAG: surface=api
        );
        assert_eq!(
            SessionError::RefreshTokenReuse.to_string(),
            "Refresh token reuse detected"
        );
        assert_eq!(
            SessionError::RateLimitExceeded.to_string(),
            "Rate limit exceeded for token refresh"
        );
        assert_eq!(
            SessionError::EncryptionError("decrypt fail".to_string()).to_string(),
            "Encryption error: decrypt fail"
        );
    }

    #[test]
    fn test_session_error_from_libsql() {
        let libsql_err = libsql::Error::SqliteFailure(1, "no such table".to_string());
        let err: SessionError = libsql_err.into();
        assert!(matches!(err, SessionError::DatabaseError(_)));
        assert!(err.to_string().contains("no such table"));
    }

    #[test]
    fn test_session_error_from_serde_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("{bad").unwrap_err();
        let err: SessionError = json_err.into();
        assert!(matches!(err, SessionError::SerializationError(_)));
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
