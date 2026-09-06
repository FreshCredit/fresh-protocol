
// TAG: surface=security owner=security-team rule=SEC-001
//! Token and refresh operations for LibSQL session storage

use super::core::LibSqlSessionStorage;
use crate::storage::{Session, SessionError, SessionStorage};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use tracing::debug;
use uuid::Uuid;

#[async_trait]
impl SessionStorage for LibSqlSessionStorage {
    async fn store_refresh_token(
        &self,
        session_id: &str,
        refresh_token: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), SessionError> {
        let query = r#"
            UPDATE sessions
            SET refresh_token_encrypted = ?,
                refresh_token_expires_at = ?
            WHERE id = ?
        "#;

        self.conn
            .execute(
                query,
                libsql::params![refresh_token, expires_at.timestamp(), session_id,],
            )
            .await?;
        // TAG: surface=security owner=platform-team rule=MID-001

        debug!("Stored refresh token for session {}", session_id);
        Ok(())
    }

    async fn get_refresh_token(&self, session_id: &str) -> Result<Option<String>, SessionError> {
        let query = r#"
            SELECT refresh_token_encrypted
            FROM sessions
            WHERE id = ? AND refresh_token_expires_at > ?
        "#;

        let now = Utc::now().timestamp();
        let mut rows = self
            .conn
            .query(query, libsql::params![session_id, now])
            .await?;

        if let Some(row) = rows.next().await? {
            let token: Option<String> = row.get(0)?;
            Ok(token)
        } else {
            Ok(None)
        }
    }

    async fn update_tokens(
        &self,
        session_id: &str,
        access_token: &str,
        // TAG: surface=security owner=platform-team rule=GENERAL-001
        refresh_token: Option<&str>,
        expires_in: i64,
    ) -> Result<(), SessionError> {
        let now = Utc::now();
        let new_expires_at = now + Duration::seconds(expires_in);
        let refresh_expires_at = now + Duration::days(90); // 90 days for refresh token

        let query = if refresh_token.is_some() {
            r#"
                UPDATE sessions
                SET access_token = ?,
                    refresh_token_encrypted = ?,
                    expires_at = ?,
                    refresh_token_expires_at = ?,
                    token_rotation_count = token_rotation_count + 1,
                    last_refresh_at = ?,
                    last_activity = ?
                WHERE id = ?
            "#
        } else {
            r#"
                UPDATE sessions
                SET access_token = ?,
                    expires_at = ?,
                    token_rotation_count = token_rotation_count + 1,
                    last_refresh_at = ?,
                    last_activity = ?
                WHERE id = ?
            "#
        };
                    // TAG: surface=security owner=platform-team rule=MID-001

        if let Some(ref_token) = refresh_token {
            self.conn
                .execute(
                    query,
                    libsql::params![
                        access_token,
                        ref_token,
                        new_expires_at.timestamp(),
                        refresh_expires_at.timestamp(),
                        now.timestamp(),
                        now.timestamp(),
                        session_id,
                    ],
                )
                .await?;
        } else {
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
        // TAG: surface=security owner=security-team rule=SEC-001

        debug!("Updated tokens for session {}", session_id);
        Ok(())
    }

    async fn get_by_access_token(
        &self,
        access_token: &str,
    ) -> Result<Option<Session>, SessionError> {
        let query = r#"
            SELECT id, user_id, created_at, last_activity, expires_at, data,
                   user_agent, ip_address, device_type, os, browser, country,
                   access_token, refresh_token_encrypted, refresh_token_expires_at,
                   token_rotation_count, last_refresh_at
            FROM sessions
            WHERE access_token = ? AND expires_at > ?
        "#;

        let now = Utc::now().timestamp();
        let mut rows = self
            .conn
            .query(query, libsql::params![access_token, now])
            .await?;

        if let Some(row) = rows.next().await? {
            LibSqlSessionStorage::row_to_session(&row)
        } else {
            Ok(None)
        }
    }
        // TAG: surface=security owner=platform-team rule=MID-001

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
        // P1-003: Table creation moved to initialization to prevent TOCTOU race condition
        // The refresh_token_events table should be created during app startup/migration
        // not on-demand during runtime. This ensures all audit events are captured.

        let query = r#"
            INSERT INTO refresh_token_events (id, session_id, user_id, event_type,
                                              ip_address, user_agent, success, error_message, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();

        self.conn
            .execute(
                query,
                libsql::params![
                    id.clone(),
                    session_id,
                    // TAG: surface=security owner=platform-team rule=GENERAL-001
                    user_id,
                    event_type,
                    ip_address,
                    user_agent,
                    success,
                    error_message,
                    now,
                ],
            )
            .await?;

        debug!(
            "Recorded refresh event {} for session {}: {} (success: {})",
            id, session_id, event_type, success
        );
        Ok(())
    }

    async fn check_refresh_rate_limit(
        &self,
        session_id: &str,
        ip_address: &str,
    ) -> Result<bool, SessionError> {
        // First ensure the rate limit table exists
        let create_table = r#"
            CREATE TABLE IF NOT EXISTS refresh_rate_limits (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                ip_address TEXT NOT NULL,
                attempt_count INTEGER DEFAULT 1,
                first_attempt_at INTEGER NOT NULL,
                last_attempt_at INTEGER NOT NULL,
                blocked_until INTEGER,
                UNIQUE(session_id, ip_address)
            )
        "#;
        let _ = self.conn.execute(create_table, ()).await; // AUDIT-OK(fire-and-forget): lazy idempotent DDL; failure surfaces in the follow-up query

        let query = r#"
            SELECT attempt_count, blocked_until
            FROM refresh_rate_limits
            WHERE session_id = ? AND ip_address = ?
        "#;

        let mut rows = self
            .conn
            .query(query, libsql::params![session_id, ip_address])
            .await?;

        if let Some(row) = rows.next().await? {
            let attempt_count: i32 = row.get(0)?;
            let blocked_until: Option<i64> = row.get(1)?;
            let now = Utc::now().timestamp();

            // Check if still blocked
            if let Some(blocked_ts) = blocked_until {
                if now < blocked_ts {
                    return Ok(false); // Still blocked
                }
            }
            // TAG: surface=security owner=security-team rule=SEC-001

            // Allow if under limit (10 attempts per 5 minutes)
            Ok(attempt_count < 10)
        } else {
            Ok(true) // No record yet, allow
        }
    }

    async fn increment_refresh_rate_limit(
        &self,
        session_id: &str,
        ip_address: &str,
    ) -> Result<(), SessionError> {
        let now = Utc::now().timestamp();
        let five_minutes = 5 * 60;
        let blocked_until = now + five_minutes;

        let query = r#"
            INSERT INTO refresh_rate_limits (id, session_id, ip_address, attempt_count,
                                             first_attempt_at, last_attempt_at, blocked_until)
            VALUES (?, ?, ?, 1, ?, ?, NULL)
            ON CONFLICT(session_id, ip_address) DO UPDATE SET
                attempt_count = CASE
                    WHEN last_attempt_at < ? - ? THEN 1
                    ELSE attempt_count + 1
                END,
                first_attempt_at = CASE
                    WHEN last_attempt_at < ? - ? THEN ?
                    ELSE first_attempt_at
                END,
                last_attempt_at = ?,
                blocked_until = CASE
                    WHEN attempt_count >= 10 THEN ?
                    ELSE NULL
                END
        "#;

        self.conn
            .execute(
                query,
                libsql::params![
                    Uuid::new_v4().to_string(),
                    session_id,
                    ip_address,
                    now,
                    now,
                    now,
                    five_minutes,
                    now,
                    five_minutes,
                    now,
                    now,
                    blocked_until,
                ],
            )
            .await?;

        Ok(())
    }
// TAG: surface=security owner=platform-team rule=GENERAL-001
}
