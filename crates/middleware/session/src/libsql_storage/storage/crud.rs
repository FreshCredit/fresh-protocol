// TAG: surface=security owner=security-team rule=SEC-001
use crate::libsql_storage::core::LibSqlSessionStorage;
use crate::libsql_storage::token_crypto::encrypt_optional_refresh_token;
use crate::storage::{Session, SessionError};
use tracing::debug;

impl LibSqlSessionStorage {
    pub(crate) async fn create_impl(&self, session: Session) -> Result<(), SessionError> {
        let query = r"
            INSERT INTO sessions (id, user_id, created_at, last_activity, expires_at, data,
                                 user_agent, ip_address, device_type, os, browser, country,
                                 access_token, refresh_token_encrypted, refresh_token_expires_at,
                                 token_rotation_count, last_refresh_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ";

        let data_json = serde_json::to_string(&session.data)?;
        let session_id = session.id.clone();
        let user_id = session.user_id.clone();
        let device = &session.device_info;
        // Encrypt the OAuth refresh token before it touches the database
        // (no-op passthrough when encryption is not configured).
        let refresh_token_encrypted =
            encrypt_optional_refresh_token(session.refresh_token.as_deref());

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
                    refresh_token_encrypted,
                    session.refresh_token_expires_at.map(|d| d.timestamp()),
                    session.token_rotation_count,
                    session.last_refresh_at.map(|d| d.timestamp()),
                ],
            )
            .await?;

        debug!("Created session {} for user {}", session_id, user_id);
        Ok(())
    }
}
