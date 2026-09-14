// TAG: surface=security owner=security-team rule=SEC-001
use crate::libsql_storage::core::LibSqlSessionStorage;
use crate::libsql_storage::token_crypto::encrypt_refresh_token_for_storage;
use crate::storage::SessionError;
use chrono::{Duration, Utc};
use tracing::debug;

impl LibSqlSessionStorage {
    pub(crate) async fn update_tokens_impl(
        &self,
        session_id: &str,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: i64,
    ) -> Result<(), SessionError> {
        let now = Utc::now();
        let new_expires_at = now + Duration::seconds(expires_in);
        let refresh_expires_at = now + Duration::days(90);

        let query = if refresh_token.is_some() {
            // TAG: surface=security owner=security-team rule=SEC-001
            r"
                UPDATE sessions
                SET access_token = ?,
                    refresh_token_encrypted = ?,
                    expires_at = ?,
                    refresh_token_expires_at = ?,
                    token_rotation_count = token_rotation_count + 1,
                    last_refresh_at = ?,
                    last_activity = ?
                WHERE id = ?
            "
        } else {
            r"
                UPDATE sessions
                SET access_token = ?,
                    expires_at = ?,
                    token_rotation_count = token_rotation_count + 1,
                    last_refresh_at = ?,
                    last_activity = ?
                WHERE id = ?
            "
        };

        if let Some(ref_token) = refresh_token {
            let encrypted = encrypt_refresh_token_for_storage(ref_token);
            self.conn
                .execute(
                    query,
                    libsql::params![
                        access_token,
                        encrypted,
                        new_expires_at.timestamp(),
                        refresh_expires_at.timestamp(),
                        now.timestamp(),
                        now.timestamp(),
                        session_id,
                    ],
                )
                .await?;
        } else {
            // TAG: surface=security owner=platform-team rule=MID-001
            self.conn
                .execute(
                    query,
                    libsql::params![
                        access_token,
                        new_expires_at.timestamp(),
                        now.timestamp(),
                        now.timestamp(),
                        session_id,
                    ],
                )
                .await?;
        }

        debug!("Updated tokens for session {}", session_id);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn record_refresh_event_impl(
        &self,
        session_id: &str,
        user_id: &str,
        event_type: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        success: bool,
        error_message: Option<&str>,
        // TAG: surface=security owner=platform-team rule=MID-001
    ) -> Result<(), SessionError> {
        let query = r"
            INSERT INTO refresh_events (id, session_id, user_id, event_type, ip_address, user_agent, success, error_message, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ";

        self.conn
            .execute(
                query,
                libsql::params![
                    uuid::Uuid::new_v4().to_string(),
                    session_id,
                    user_id,
                    event_type,
                    ip_address,
                    user_agent,
                    success,
                    error_message,
                    Utc::now().timestamp(),
                ],
            )
            .await?;

        debug!(
            "Recorded refresh event {} for session {}",
            event_type, session_id
        );
        Ok(())
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
