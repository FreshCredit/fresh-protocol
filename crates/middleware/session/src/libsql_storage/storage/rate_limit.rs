// TAG: surface=security owner=security-team rule=SEC-001
use crate::libsql_storage::core::LibSqlSessionStorage;
use crate::storage::SessionError;
use chrono::{Duration, Utc};
use tracing::{debug, info};

impl LibSqlSessionStorage {
    pub(crate) async fn check_refresh_rate_limit_impl(
        &self,
        session_id: &str,
        ip_address: &str,
    ) -> Result<bool, SessionError> {
        let query = r"
            SELECT COUNT(*) as count, MAX(created_at) as last_attempt
            FROM refresh_rate_limits
            WHERE (session_id = ? OR ip_address = ?)
            AND created_at > ?
        ";

        let window_start = (Utc::now() - Duration::minutes(5)).timestamp();

        let mut rows = self
            .conn
            .query(query, libsql::params![session_id, ip_address, window_start])
            .await?;

        let (count, _last_attempt) = if let Some(row) = rows.next().await? {
            (row.get(0)?, row.get(1)?)
        } else {
            (0i64, None::<i64>)
        };

        let max_attempts = 10;
        if count >= max_attempts {
            info!(
                "Rate limit exceeded for session {} / ip {} ({} attempts)",
                session_id, ip_address, count
            );
            Ok(false)
        } else {
            // TAG: surface=security owner=platform-team rule=MID-001
            debug!(
                "Rate limit check passed for session {} / ip {} ({} attempts)",
                session_id, ip_address, count
            );
            Ok(true)
        }
    }

    pub(crate) async fn increment_refresh_rate_limit_impl(
        &self,
        session_id: &str,
        ip_address: &str,
    ) -> Result<(), SessionError> {
        let query = r"
            INSERT INTO refresh_rate_limits (id, session_id, ip_address, created_at, expires_at)
            VALUES (?, ?, ?, ?, ?)
        ";

        let now = Utc::now();
        let expires_at = now + Duration::minutes(5);

        self.conn
            .execute(
                query,
                libsql::params![
                    uuid::Uuid::new_v4().to_string(),
                    session_id,
                    ip_address,
                    now.timestamp(),
                    expires_at.timestamp(),
                ],
            )
            .await?;

        debug!(
            "Incremented rate limit for session {} / ip {}",
            session_id, ip_address
        );
        Ok(())
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
