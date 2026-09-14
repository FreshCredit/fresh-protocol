// TAG: surface=security owner=security-team rule=SEC-001
//! `SessionStorage` trait implementation for `LibSQL` session storage

/// Session artifact storage
pub mod artifacts;
/// Session CRUD operations
pub mod crud;
/// Session rate limiting
pub mod rate_limit;
/// Session token storage
pub mod tokens;

use crate::libsql_storage::core::LibSqlSessionStorage;
use crate::libsql_storage::token_crypto::{
    decrypt_refresh_token_from_storage, encrypt_refresh_token_for_storage,
};
use crate::storage::{Session, SessionArtifact, SessionError, SessionStorage};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use tracing::{debug, info};

#[async_trait]
impl SessionStorage for LibSqlSessionStorage {
    async fn create(&self, session: Session) -> Result<(), SessionError> {
        self.create_impl(session).await
    }

    async fn get(&self, session_id: &str) -> Result<Option<Session>, SessionError> {
        let query = r"
            SELECT id, user_id, created_at, last_activity, expires_at, data,
                   user_agent, ip_address, device_type, os, browser, country,
                   access_token, refresh_token_encrypted, refresh_token_expires_at,
                   token_rotation_count, last_refresh_at
            FROM sessions
            WHERE id = ?
        ";

        let mut rows = self.conn.query(query, libsql::params![session_id]).await?;

        rows.next()
            .await?
            .map_or(Ok(None), |row| Self::row_to_session(&row))
    }

    async fn get_and_refresh(
        &self,
        session_id: &str,
        timeout: Duration,
        // TAG: surface=security owner=platform-team rule=MID-001
    ) -> Result<Option<Session>, SessionError> {
        let now = Utc::now();
        let new_expires_at = now + timeout;

        let query = r"
            UPDATE sessions
            SET last_activity = ?, expires_at = ?
            WHERE id = ?
            RETURNING id, user_id, created_at, last_activity, expires_at, data,
                      user_agent, ip_address, device_type, os, browser, country,
                      access_token, refresh_token_encrypted, refresh_token_expires_at,
                      token_rotation_count, last_refresh_at
        ";

        let mut rows = self
            .conn
            .query(
                query,
                libsql::params![now.timestamp(), new_expires_at.timestamp(), session_id],
            )
            .await?;

        rows.next()
            .await?
            .map_or(Ok(None), |row| Self::row_to_session(&row))
    }

    async fn update_activity(
        &self,
        session_id: &str,
        timeout: Duration,
    ) -> Result<(), SessionError> {
        let now = Utc::now();
        let new_expires_at = now + timeout;

        let query = r"
            UPDATE sessions
            SET last_activity = ?, expires_at = ?
            WHERE id = ?
        ";

        self.conn
            .execute(
                query,
                libsql::params![now.timestamp(), new_expires_at.timestamp(), session_id],
                // TAG: surface=security owner=platform-team rule=MID-001
            )
            .await?;

        debug!("Updated activity for session {}", session_id);
        Ok(())
    }

    async fn delete(&self, session_id: &str) -> Result<(), SessionError> {
        let query = "DELETE FROM sessions WHERE id = ?";
        self.conn
            .execute(query, libsql::params![session_id])
            .await?;
        debug!("Deleted session {}", session_id);
        Ok(())
    }

    async fn cleanup_expired(&self) -> Result<u64, SessionError> {
        let now = Utc::now().timestamp();
        let query = "DELETE FROM sessions WHERE expires_at < ?";
        let rows_affected = self.conn.execute(query, libsql::params![now]).await?;
        info!("Cleaned up {} expired sessions", rows_affected);
        Ok(rows_affected)
    }

    async fn get_user_sessions(&self, user_id: &str) -> Result<Vec<Session>, SessionError> {
        let query = r"
            SELECT id, user_id, created_at, last_activity, expires_at, data,
                   user_agent, ip_address, device_type, os, browser, country,
                   access_token, refresh_token_encrypted, refresh_token_expires_at,
                   token_rotation_count, last_refresh_at
            FROM sessions
            WHERE user_id = ?
            ORDER BY last_activity DESC
        ";

        let mut rows = self.conn.query(query, libsql::params![user_id]).await?;
        let mut sessions = Vec::new();

        while let Some(row) = rows.next().await? {
            if let Some(session) = Self::row_to_session(&row)? {
                sessions.push(session);
            }
        }

        debug!("Retrieved {} sessions for user {}", sessions.len(), user_id);
        Ok(sessions)
    }
    // TAG: surface=security owner=platform-team rule=MID-001

    async fn record_artifact(&self, artifact: SessionArtifact) -> Result<(), SessionError> {
        self.record_artifact_impl(artifact).await
    }

    async fn get_session_artifacts(
        &self,
        session_id: &str,
    ) -> Result<Vec<SessionArtifact>, SessionError> {
        let query = r"
            SELECT id, session_id, user_id, artifact_type, data, created_at
            FROM session_artifacts
            WHERE session_id = ?
            ORDER BY created_at DESC
        ";

        let mut rows = self.conn.query(query, libsql::params![session_id]).await?;
        let mut artifacts = Vec::new();

        while let Some(row) = rows.next().await? {
            artifacts.push(Self::parse_artifact_row(&row)?);
        }

        debug!(
            "Retrieved {} artifacts for session {}",
            artifacts.len(),
            session_id
        );
        Ok(artifacts)
    }

    async fn get_user_artifacts(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<SessionArtifact>, SessionError> {
        let query = r"
            SELECT id, session_id, user_id, artifact_type, data, created_at
            FROM session_artifacts
            WHERE user_id = ?
            ORDER BY created_at DESC
            LIMIT ?
        ";

        let mut rows = self
            .conn
            .query(query, libsql::params![user_id, limit])
            // TAG: surface=security owner=platform-team rule=MID-001
            .await?;
        let mut artifacts = Vec::new();

        while let Some(row) = rows.next().await? {
            artifacts.push(Self::parse_artifact_row(&row)?);
        }

        debug!(
            "Retrieved {} artifacts for user {}",
            artifacts.len(),
            user_id
        );
        Ok(artifacts)
    }

    async fn store_refresh_token(
        &self,
        session_id: &str,
        refresh_token: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), SessionError> {
        let query = r"
            UPDATE sessions
            SET refresh_token_encrypted = ?, refresh_token_expires_at = ?
            WHERE id = ?
        ";

        let encrypted = encrypt_refresh_token_for_storage(refresh_token);
        self.conn
            .execute(
                query,
                libsql::params![encrypted, expires_at.timestamp(), session_id],
            )
            .await?;

        debug!("Stored refresh token for session {}", session_id);
        Ok(())
    }

    async fn get_refresh_token(&self, session_id: &str) -> Result<Option<String>, SessionError> {
        let query = r"
            SELECT refresh_token_encrypted
            FROM sessions
            WHERE id = ? AND refresh_token_expires_at > ?
        ";

        let now = Utc::now().timestamp();
        let mut rows = self
            // TAG: surface=security owner=platform-team rule=MID-001
            .conn
            .query(query, libsql::params![session_id, now])
            .await?;

        if let Some(row) = rows.next().await? {
            let token: Option<String> = row.get(0)?;
            // Decrypt on read; legacy plaintext values pass through unchanged.
            Ok(token.map(|t| decrypt_refresh_token_from_storage(&t)))
        } else {
            Ok(None)
        }
    }

    async fn update_tokens(
        &self,
        session_id: &str,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: i64,
    ) -> Result<(), SessionError> {
        self.update_tokens_impl(session_id, access_token, refresh_token, expires_in)
            .await
    }

    async fn get_by_access_token(
        &self,
        access_token: &str,
    ) -> Result<Option<Session>, SessionError> {
        let query = r"
            SELECT id, user_id, created_at, last_activity, expires_at, data,
                   user_agent, ip_address, device_type, os, browser, country,
                   access_token, refresh_token_encrypted, refresh_token_expires_at,
                   token_rotation_count, last_refresh_at
            FROM sessions
            WHERE access_token = ?
        ";

        let mut rows = self
            .conn
            .query(query, libsql::params![access_token])
            .await?;

        rows.next()
            .await?
            .map_or(Ok(None), |row| Self::row_to_session(&row))
    }
    // TAG: surface=security owner=platform-team rule=MID-001

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
    ) -> Result<(), SessionError> {
        self.record_refresh_event_impl(
            session_id,
            user_id,
            event_type,
            ip_address,
            user_agent,
            success,
            error_message,
        )
        .await
    }

    async fn check_refresh_rate_limit(
        &self,
        session_id: &str,
        ip_address: &str,
    ) -> Result<bool, SessionError> {
        self.check_refresh_rate_limit_impl(session_id, ip_address)
            .await
    }

    async fn increment_refresh_rate_limit(
        &self,
        session_id: &str,
        ip_address: &str,
    ) -> Result<(), SessionError> {
        self.increment_refresh_rate_limit_impl(session_id, ip_address)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{SessionArtifact, SessionArtifactType};
    // TAG: surface=security owner=platform-team rule=MID-001
    use std::sync::Arc;

    async fn in_memory_storage() -> LibSqlSessionStorage {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .unwrap();
        let conn = db.connect().unwrap();
        let storage = LibSqlSessionStorage::new(Arc::new(conn));
        storage.init().await.unwrap();

        // Create tables not created by init()
        storage
            .conn
            .execute(
                "CREATE TABLE IF NOT EXISTS refresh_events (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    user_id TEXT NOT NULL,
                    event_type TEXT NOT NULL,
                    ip_address TEXT,
                    user_agent TEXT,
                    success INTEGER NOT NULL,
                    error_message TEXT,
                    created_at INTEGER NOT NULL
                )",
                (),
            )
            .await
            .unwrap();

        storage
            .conn
            .execute(
                "CREATE TABLE IF NOT EXISTS refresh_rate_limits (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    ip_address TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    expires_at INTEGER NOT NULL
                )",
                (),
            )
            .await
            .unwrap();

        storage
        // TAG: surface=security owner=platform-team rule=MID-001
    }

    fn test_session(user_id: &str) -> Session {
        Session::new(
            user_id.to_string(),
            Duration::minutes(30),
            Duration::hours(24),
        )
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let storage = in_memory_storage().await;
        let session = test_session("user_1");
        let session_id = session.id.clone();

        storage.create(session.clone()).await.unwrap();

        let retrieved = storage.get(&session_id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, session_id);
        assert_eq!(retrieved.user_id, "user_1");
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let storage = in_memory_storage().await;
        let result = storage.get("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_and_refresh() {
        let storage = in_memory_storage().await;
        let session = test_session("user_1");
        let session_id = session.id.clone();
        let original_expires_ts = session.expires_at.timestamp();

        storage.create(session).await.unwrap();

        let refreshed = storage
            .get_and_refresh(&session_id, Duration::minutes(30))
            .await
            .unwrap();
        assert!(refreshed.is_some());
        let refreshed = refreshed.unwrap();
        // TAG: surface=security owner=platform-team rule=MID-001
        assert_eq!(refreshed.id, session_id);
        // Expires_at should be updated (compare timestamps since DB truncates to seconds)
        assert!(refreshed.expires_at.timestamp() >= original_expires_ts);
    }

    #[tokio::test]
    async fn test_get_and_refresh_not_found() {
        let storage = in_memory_storage().await;
        let result = storage
            .get_and_refresh("nonexistent", Duration::minutes(30))
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_update_activity() {
        let storage = in_memory_storage().await;
        let session = test_session("user_1");
        let session_id = session.id.clone();
        let original_activity_ts = session.last_activity.timestamp();
        let original_expires_ts = session.expires_at.timestamp();

        storage.create(session).await.unwrap();
        storage
            .update_activity(&session_id, Duration::minutes(30))
            .await
            .unwrap();

        let retrieved = storage.get(&session_id).await.unwrap().unwrap();
        assert!(retrieved.last_activity.timestamp() >= original_activity_ts);
        assert!(retrieved.expires_at.timestamp() >= original_expires_ts);
    }

    #[tokio::test]
    async fn test_delete() {
        let storage = in_memory_storage().await;
        let session = test_session("user_1");
        let session_id = session.id.clone();

        storage.create(session).await.unwrap();
        storage.delete(&session_id).await.unwrap();

        let retrieved = storage.get(&session_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    // TAG: surface=security owner=platform-team rule=MID-001
    #[tokio::test]
    async fn test_cleanup_expired() {
        let storage = in_memory_storage().await;

        // Create an expired session
        let mut session = test_session("user_1");
        session.expires_at = Utc::now() - Duration::minutes(1);
        let session_id = session.id.clone();

        storage.create(session).await.unwrap();

        // Create a non-expired session
        let session2 = test_session("user_2");
        let session_id2 = session2.id.clone();
        storage.create(session2).await.unwrap();

        let deleted = storage.cleanup_expired().await.unwrap();
        assert_eq!(deleted, 1);

        assert!(storage.get(&session_id).await.unwrap().is_none());
        assert!(storage.get(&session_id2).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_get_user_sessions() {
        let storage = in_memory_storage().await;
        let s1 = test_session("user_1");
        let s2 = test_session("user_1");
        let s3 = test_session("user_2");

        storage.create(s1).await.unwrap();
        storage.create(s2).await.unwrap();
        storage.create(s3).await.unwrap();

        let sessions = storage.get_user_sessions("user_1").await.unwrap();
        assert_eq!(sessions.len(), 2);

        let sessions = storage.get_user_sessions("user_2").await.unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[tokio::test]
    async fn test_record_and_get_session_artifacts() {
        let storage = in_memory_storage().await;
        let session = test_session("user_1");
        let session_id = session.id.clone();
        storage.create(session).await.unwrap();
        // TAG: surface=security owner=platform-team rule=MID-001

        let artifact = SessionArtifact::login(
            session_id.clone(),
            "user_1".to_string(),
            Some("127.0.0.1".to_string()),
        );
        storage.record_artifact(artifact).await.unwrap();

        let artifacts = storage.get_session_artifacts(&session_id).await.unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].artifact_type, SessionArtifactType::Login);
    }

    #[tokio::test]
    async fn test_get_user_artifacts() {
        let storage = in_memory_storage().await;
        let session = test_session("user_1");
        let session_id = session.id.clone();
        storage.create(session).await.unwrap();

        let artifact1 = SessionArtifact::login(session_id.clone(), "user_1".to_string(), None);
        let artifact2 = SessionArtifact::logout(session_id.clone(), "user_1".to_string());
        storage.record_artifact(artifact1).await.unwrap();
        storage.record_artifact(artifact2).await.unwrap();

        let user_artifacts = storage.get_user_artifacts("user_1", 10).await.unwrap();
        assert_eq!(user_artifacts.len(), 2);

        // Test limit
        let limited_artifacts = storage.get_user_artifacts("user_1", 1).await.unwrap();
        assert_eq!(limited_artifacts.len(), 1);
    }

    #[tokio::test]
    async fn test_store_and_get_refresh_token() {
        let storage = in_memory_storage().await;
        let session = test_session("user_1");
        let session_id = session.id.clone();
        storage.create(session).await.unwrap();

        // No token initially
        let token = storage.get_refresh_token(&session_id).await.unwrap();
        assert!(token.is_none());

        // Store a token
        let expires_at = Utc::now() + Duration::hours(1);
        storage
            // TAG: surface=security owner=platform-team rule=MID-001
            .store_refresh_token(&session_id, "my_refresh_token", expires_at)
            .await
            .unwrap();

        let token = storage.get_refresh_token(&session_id).await.unwrap();
        assert_eq!(token, Some("my_refresh_token".to_string()));
    }

    #[tokio::test]
    async fn test_get_refresh_token_reads_legacy_plaintext() {
        // Sessions persisted before at-rest encryption store plaintext in
        // `refresh_token_encrypted`; reads must pass them through unchanged.
        let storage = in_memory_storage().await;
        let session = test_session("user_1");
        let session_id = session.id.clone();
        storage.create(session).await.unwrap();

        let expires_at = (Utc::now() + Duration::hours(1)).timestamp();
        storage
            .conn
            .execute(
                "UPDATE sessions SET refresh_token_encrypted = ?, refresh_token_expires_at = ? WHERE id = ?",
                libsql::params!["legacy-plaintext-token", expires_at, session_id.clone()],
            )
            .await
            .unwrap();

        let token = storage.get_refresh_token(&session_id).await.unwrap();
        assert_eq!(token, Some("legacy-plaintext-token".to_string()));

        // The full row read path must also surface the legacy value.
        let retrieved = storage.get(&session_id).await.unwrap().unwrap();
        assert_eq!(
            retrieved.refresh_token,
            Some("legacy-plaintext-token".to_string())
        );
    }

    #[tokio::test]
    async fn test_get_refresh_token_expired() {
        let storage = in_memory_storage().await;
        let session = test_session("user_1");
        let session_id = session.id.clone();
        storage.create(session).await.unwrap();

        // Store an expired token
        let expires_at = Utc::now() - Duration::minutes(1);
        storage
            .store_refresh_token(&session_id, "expired_token", expires_at)
            .await
            .unwrap();

        let token = storage.get_refresh_token(&session_id).await.unwrap();
        assert!(token.is_none());
    }

    #[tokio::test]
    async fn test_update_tokens_with_refresh() {
        let storage = in_memory_storage().await;
        let session = test_session("user_1");
        let session_id = session.id.clone();
        storage.create(session).await.unwrap();

        storage
            .update_tokens(&session_id, "new_access", Some("new_refresh"), 3600)
            .await
            .unwrap();

        let retrieved = storage.get(&session_id).await.unwrap().unwrap();
        assert_eq!(retrieved.access_token, Some("new_access".to_string()));
        assert_eq!(retrieved.refresh_token, Some("new_refresh".to_string()));
        assert_eq!(retrieved.token_rotation_count, 1);
        assert!(retrieved.last_refresh_at.is_some());
    }

    #[tokio::test]
    async fn test_update_tokens_without_refresh() {
        // TAG: surface=security owner=platform-team rule=MID-001
        let storage = in_memory_storage().await;
        let session = test_session("user_1");
        let session_id = session.id.clone();
        storage.create(session).await.unwrap();

        storage
            .update_tokens(&session_id, "new_access", None, 3600)
            .await
            .unwrap();

        let retrieved = storage.get(&session_id).await.unwrap().unwrap();
        assert_eq!(retrieved.access_token, Some("new_access".to_string()));
        assert_eq!(retrieved.token_rotation_count, 1);
    }

    #[tokio::test]
    async fn test_get_by_access_token() {
        let storage = in_memory_storage().await;
        let mut session = test_session("user_1");
        session.access_token = Some("token_123".to_string());
        let session_id = session.id.clone();
        storage.create(session).await.unwrap();

        let found = storage.get_by_access_token("token_123").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, session_id);

        let not_found = storage.get_by_access_token("wrong_token").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_record_refresh_event() {
        let storage = in_memory_storage().await;
        storage
            .record_refresh_event(
                "session_1",
                "user_1",
                "token_refresh",
                Some("127.0.0.1"),
                Some("Mozilla/5.0"),
                true,
                None,
            )
            .await
            .unwrap();

        // TAG: surface=security owner=platform-team rule=MID-001
        // Verify it was inserted
        let mut rows = storage
            .conn
            .query(
                "SELECT COUNT(*) FROM refresh_events WHERE session_id = ?",
                libsql::params!["session_1"],
            )
            .await
            .unwrap();
        let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_check_refresh_rate_limit_under_limit() {
        let storage = in_memory_storage().await;
        let allowed = storage
            .check_refresh_rate_limit("session_1", "127.0.0.1")
            .await
            .unwrap();
        assert!(allowed);
    }

    #[tokio::test]
    async fn test_check_refresh_rate_limit_over_limit() {
        let storage = in_memory_storage().await;

        // Insert 10 rate limit entries
        for _ in 0..10 {
            storage
                .increment_refresh_rate_limit("session_1", "127.0.0.1")
                .await
                .unwrap();
        }

        let allowed = storage
            .check_refresh_rate_limit("session_1", "127.0.0.1")
            .await
            .unwrap();
        assert!(!allowed);
    }

    #[tokio::test]
    async fn test_increment_refresh_rate_limit() {
        let storage = in_memory_storage().await;
        storage
            .increment_refresh_rate_limit("session_1", "127.0.0.1")
            // TAG: surface=security owner=platform-team rule=MID-001
            .await
            .unwrap();

        let mut rows = storage
            .conn
            .query(
                "SELECT COUNT(*) FROM refresh_rate_limits WHERE session_id = ?",
                libsql::params!["session_1"],
            )
            .await
            .unwrap();
        let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_session_with_device_info_roundtrip() {
        let storage = in_memory_storage().await;
        let mut session = Session::with_device_info(
            "user_1".to_string(),
            Duration::minutes(30),
            Duration::hours(24),
            Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64)".to_string()),
            Some("192.168.1.1".to_string()),
        );
        session.device_info.country = Some("US".to_string());
        let session_id = session.id.clone();

        storage.create(session).await.unwrap();

        let retrieved = storage.get(&session_id).await.unwrap().unwrap();
        assert_eq!(
            retrieved.device_info.user_agent,
            Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64)".to_string())
        );
        assert_eq!(
            retrieved.device_info.ip_address,
            Some("192.168.1.1".to_string())
        );
        assert_eq!(
            retrieved.device_info.device_type,
            Some("desktop".to_string())
        );
        assert_eq!(retrieved.device_info.os, Some("Windows".to_string()));
        assert_eq!(retrieved.device_info.browser, Some("Unknown".to_string()));
        assert_eq!(retrieved.device_info.country, Some("US".to_string()));
    }
}
