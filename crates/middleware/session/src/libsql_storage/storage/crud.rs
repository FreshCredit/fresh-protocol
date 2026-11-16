// TAG: surface=security owner=security-team rule=SEC-001
use crate::libsql_storage::core::LibSqlSessionStorage;
use crate::libsql_storage::token_crypto::try_encrypt_refresh_token_with_context;
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
        // Encrypt the OAuth refresh token bound to (session_id, user_id) AAD before storage (fail closed in prod)
        let refresh_token_encrypted = session
            .refresh_token
            .as_deref()
            .map(|t| try_encrypt_refresh_token_with_context(&session.id, &session.user_id, t))
            .transpose()
            .map_err(|e| SessionError::InvalidData(e.to_string()))?;

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
