// TAG: surface=security owner=security-team rule=SEC-001
//! Core `LibSQL` session storage types and helpers

use crate::libsql_storage::token_crypto::decrypt_optional_refresh_token;
use crate::storage::{DeviceInfo, Session, SessionArtifact, SessionArtifactType, SessionError};
use chrono::{DateTime, Utc};
use libsql::Connection;
use std::sync::Arc;

/// `LibSQL` session storage implementation
#[derive(Debug)]
pub struct LibSqlSessionStorage {
    pub(crate) conn: Arc<Connection>,
}

impl LibSqlSessionStorage {
    /// Create a new `LibSQL` session storage
    #[must_use]
    pub const fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }

    /// Helper function to convert a database row to a Session
    pub(crate) fn row_to_session(row: &libsql::Row) -> Result<Option<Session>, SessionError> {
        let fields = extract_session_fields(row)?;

        Ok(Some(Session {
            id: fields.id,
            user_id: fields.user_id,
            created_at: parse_session_timestamp(fields.created_at, "created_at")?,
            last_activity: parse_session_timestamp(fields.last_activity, "last_activity")?,
            expires_at: parse_session_timestamp(fields.expires_at, "expires_at")?,
            data: serde_json::from_str(&fields.data_json)?,
            // TAG: surface=security owner=platform-team rule=MID-001
            device_info: DeviceInfo {
                user_agent: fields.user_agent,
                ip_address: fields.ip_address,
                device_type: fields.device_type,
                os: fields.os,
                browser: fields.browser,
                country: fields.country,
            },
            access_token: fields.access_token,
            // Decrypt the stored refresh token (legacy plaintext passes through).
            refresh_token: decrypt_optional_refresh_token(fields.refresh_token),
            refresh_token_expires_at: fields
                .refresh_token_expires_at
                .and_then(|ts| DateTime::from_timestamp(ts, 0)),
            token_rotation_count: fields.token_rotation_count,
            last_refresh_at: fields
                .last_refresh_at
                .and_then(|ts| DateTime::from_timestamp(ts, 0)),
        }))
    }

    /// Parse a session artifact from a database row
    pub(crate) fn parse_artifact_row(row: &libsql::Row) -> Result<SessionArtifact, SessionError> {
        let id: String = row.get(0)?;
        let session_id: String = row.get(1)?;
        let user_id: String = row.get(2)?;
        let artifact_type_str: String = row.get(3)?;
        let data_json: String = row.get(4)?;
        let created_at: i64 = row.get(5)?;

        let artifact_type = match artifact_type_str.as_str() {
            "login" => SessionArtifactType::Login,
            "logout" => SessionArtifactType::Logout,
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            "security_event" => SessionArtifactType::SecurityEvent,
            "metadata" => SessionArtifactType::Metadata,
            _ => SessionArtifactType::Activity,
        };

        Ok(SessionArtifact {
            id,
            session_id,
            user_id,
            artifact_type,
            data: serde_json::from_str(&data_json)?,
            created_at: DateTime::from_timestamp(created_at, 0).ok_or_else(|| {
                SessionError::InvalidData("Invalid created_at timestamp".to_string())
            })?,
        })
    }
}

struct SessionFields {
    id: String,
    user_id: String,
    created_at: i64,
    last_activity: i64,
    expires_at: i64,
    data_json: String,
    user_agent: Option<String>,
    ip_address: Option<String>,
    device_type: Option<String>,
    os: Option<String>,
    browser: Option<String>,
    country: Option<String>,
    // TAG: surface=security owner=platform-team rule=MID-001
    access_token: Option<String>,
    refresh_token: Option<String>,
    refresh_token_expires_at: Option<i64>,
    token_rotation_count: i32,
    last_refresh_at: Option<i64>,
}

struct SessionCoreFields {
    id: String,
    user_id: String,
    created_at: i64,
    last_activity: i64,
    expires_at: i64,
    data_json: String,
}

fn extract_session_core_fields(row: &libsql::Row) -> Result<SessionCoreFields, SessionError> {
    Ok(SessionCoreFields {
        id: row.get(0)?,
        user_id: row.get(1)?,
        created_at: row.get(2)?,
        last_activity: row.get(3)?,
        expires_at: row.get(4)?,
        data_json: row.get(5)?,
    })
}

struct SessionDeviceFields {
    user_agent: Option<String>,
    ip_address: Option<String>,
    device_type: Option<String>,
    os: Option<String>,
    // TAG: surface=security owner=security-team rule=SEC-001
    browser: Option<String>,
    country: Option<String>,
}

fn extract_session_device_fields(row: &libsql::Row) -> Result<SessionDeviceFields, SessionError> {
    Ok(SessionDeviceFields {
        user_agent: row.get(6)?,
        ip_address: row.get(7)?,
        device_type: row.get(8)?,
        os: row.get(9)?,
        browser: row.get(10)?,
        country: row.get(11)?,
    })
}

struct SessionTokenFields {
    access_token: Option<String>,
    refresh_token: Option<String>,
    refresh_token_expires_at: Option<i64>,
    token_rotation_count: i32,
    last_refresh_at: Option<i64>,
}

fn extract_session_token_fields(row: &libsql::Row) -> Result<SessionTokenFields, SessionError> {
    Ok(SessionTokenFields {
        access_token: row.get(12)?,
        refresh_token: row.get(13)?,
        refresh_token_expires_at: row.get(14)?,
        token_rotation_count: row.get(15)?,
        last_refresh_at: row.get(16)?,
    })
    // TAG: surface=security owner=platform-team rule=MID-001
}

fn extract_session_fields(row: &libsql::Row) -> Result<SessionFields, SessionError> {
    let core = extract_session_core_fields(row)?;
    let device = extract_session_device_fields(row)?;
    let tokens = extract_session_token_fields(row)?;

    Ok(SessionFields {
        id: core.id,
        user_id: core.user_id,
        created_at: core.created_at,
        last_activity: core.last_activity,
        expires_at: core.expires_at,
        data_json: core.data_json,
        user_agent: device.user_agent,
        ip_address: device.ip_address,
        device_type: device.device_type,
        os: device.os,
        browser: device.browser,
        country: device.country,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        refresh_token_expires_at: tokens.refresh_token_expires_at,
        token_rotation_count: tokens.token_rotation_count,
        last_refresh_at: tokens.last_refresh_at,
    })
}

fn parse_session_timestamp(ts: i64, field: &str) -> Result<DateTime<Utc>, SessionError> {
    DateTime::from_timestamp(ts, 0)
        .ok_or_else(|| SessionError::InvalidData(format!("Invalid {field} timestamp")))
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
