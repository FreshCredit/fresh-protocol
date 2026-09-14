
// TAG: surface=security owner=security-team rule=SEC-001
//! Session CRUD operations for LibSQL session storage

use super::core::LibSqlSessionStorage;
use crate::storage::{Session, SessionError, SessionStorage};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use tracing::{debug, info};

#[async_trait]
impl SessionStorage for LibSqlSessionStorage {
    async fn create(&self, session: Session) -> Result<(), SessionError> {
        let query = r#"
            INSERT INTO sessions (id, user_id, created_at, last_activity, expires_at, data,
                                 user_agent, ip_address, device_type, os, browser, country,
                                 access_token, refresh_token_encrypted, refresh_token_expires_at,
                                 token_rotation_count, last_refresh_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        let data_json = serde_json::to_string(&session.data)?;
        let session_id = session.id.clone();
        let user_id = session.user_id.clone();
        let device = &session.device_info;

        self.conn
            .execute(
                query,
                libsql::params![
                    session.id,
                    session.user_id,
                    session.created_at.timestamp(),
                    session.last_activity.timestamp(),
                    session.expires_at.timestamp(),
                    data_json,
                    device.user_agent.clone(),
                    device.ip_address.clone(),
                    device.device_type.clone(),
                    device.os.clone(),
                    device.browser.clone(),
                    device.country.clone(),
                    session.access_token.clone(),
                    session.refresh_token.clone(),
                    session.refresh_token_expires_at.map(|d| d.timestamp()),
                    session.token_rotation_count,
                    // TAG: surface=security owner=platform-team rule=MID-001
                    session.last_refresh_at.map(|d| d.timestamp()),
                ],
            )
            .await?;

        debug!("Created session {} for user {}", session_id, user_id);
        Ok(())
    }

    async fn get(&self, session_id: &str) -> Result<Option<Session>, SessionError> {
        let query = r#"
            SELECT id, user_id, created_at, last_activity, expires_at, data,
                   user_agent, ip_address, device_type, os, browser, country,
                   access_token, refresh_token_encrypted, refresh_token_expires_at,
                   token_rotation_count, last_refresh_at
            FROM sessions
            WHERE id = ?
        "#;

        let mut rows = self.conn.query(query, libsql::params![session_id]).await?;

        if let Some(row) = rows.next().await? {
            LibSqlSessionStorage::row_to_session(&row)
        } else {
            Ok(None)
        }
    }

    /// P0-FIX: Atomically get and refresh session to prevent TOCTOU race condition
    async fn get_and_refresh(
        &self,
        session_id: &str,
        timeout: Duration,
    ) -> Result<Option<Session>, SessionError> {
        let now = Utc::now();
        let new_expires_at = now + timeout;

        // Atomic UPDATE with RETURNING - updates and returns in single operation
        let query = r#"
            UPDATE sessions
            SET last_activity = ?, expires_at = ?
            WHERE id = ?
            RETURNING id, user_id, created_at, last_activity, expires_at, data,
                      user_agent, ip_address, device_type, os, browser, country,
                      access_token, refresh_token_encrypted, refresh_token_expires_at,
                      token_rotation_count, last_refresh_at
        "#;

        let mut rows = self
            .conn
            .query(
                query,
                libsql::params![now.timestamp(), new_expires_at.timestamp(), session_id],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            LibSqlSessionStorage::row_to_session(&row)
        } else {
            Ok(None)
        }
    }

    async fn update_activity(
        &self,
        session_id: &str,
        timeout: Duration,
    ) -> Result<(), SessionError> {
        let now = Utc::now();
        let new_expires_at = now + timeout;

        let query = r#"
            UPDATE sessions
            SET last_activity = ?, expires_at = ?
            WHERE id = ?
        "#;

        self.conn
            .execute(
                query,
                libsql::params![now.timestamp(), new_expires_at.timestamp(), session_id,],
            )
            .await?;

        debug!("Updated activity for session {}", session_id);
        Ok(())
    }

            // TAG: surface=security owner=platform-team rule=MID-001
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

        let deleted = self.conn.execute(query, libsql::params![now]).await?;

        if deleted > 0 {
            info!("Cleaned up {deleted} expired sessions");
        }

        Ok(deleted)
    }

    async fn get_user_sessions(&self, user_id: &str) -> Result<Vec<Session>, SessionError> {
        let query = r#"
            SELECT id, user_id, created_at, last_activity, expires_at, data,
                   user_agent, ip_address, device_type, os, browser, country,
                   access_token, refresh_token_encrypted, refresh_token_expires_at,
                   token_rotation_count, last_refresh_at
            FROM sessions
            WHERE user_id = ?
            ORDER BY last_activity DESC
        "#;

        let mut rows = self.conn.query(query, libsql::params![user_id]).await?;
        let mut sessions = Vec::new();

        while let Some(row) = rows.next().await? {
            if let Some(session) = LibSqlSessionStorage::row_to_session(&row)? {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }
// TAG: surface=security owner=security-team rule=SEC-001
}
