
// TAG: surface=security owner=security-team rule=SEC-001
//! Session artifact operations for LibSQL session storage

use super::core::LibSqlSessionStorage;
use crate::storage::{SessionArtifact, SessionArtifactType, SessionError, SessionStorage};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tracing::debug;

#[async_trait]
impl SessionStorage for LibSqlSessionStorage {
    async fn record_artifact(&self, artifact: SessionArtifact) -> Result<(), SessionError> {
        let query = r#"
            INSERT INTO session_artifacts (id, session_id, user_id, artifact_type, data, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
        "#;

        let data_json = serde_json::to_string(&artifact.data)?;
        let artifact_type_str = match artifact.artifact_type {
            SessionArtifactType::Login => "login",
            SessionArtifactType::Logout => "logout",
            SessionArtifactType::Activity => "activity",
            SessionArtifactType::SecurityEvent => "security_event",
            SessionArtifactType::Metadata => "metadata",
        };

        let artifact_id = artifact.id.clone();
        let session_id = artifact.session_id.clone();

        self.conn
            .execute(
                query,
                libsql::params![
                    artifact.id,
                    artifact.session_id,
                    artifact.user_id,
                    artifact_type_str,
                    // TAG: surface=security owner=platform-team rule=MID-001
                    data_json,
                    artifact.created_at.timestamp(),
                ],
            )
            .await?;

        debug!(
            "Recorded artifact {} for session {}",
            artifact_id, session_id
        );
        Ok(())
    }

    async fn get_session_artifacts(
        &self,
        session_id: &str,
    ) -> Result<Vec<SessionArtifact>, SessionError> {
        let query = r#"
            SELECT id, session_id, user_id, artifact_type, data, created_at
            FROM session_artifacts
            WHERE session_id = ?
            ORDER BY created_at DESC
        "#;

        let mut rows = self.conn.query(query, libsql::params![session_id]).await?;
        let mut artifacts = Vec::new();

        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let session_id: String = row.get(1)?;
            let user_id: String = row.get(2)?;
            let artifact_type_str: String = row.get(3)?;
            let data_json: String = row.get(4)?;
            let created_at: i64 = row.get(5)?;

            let artifact_type = match artifact_type_str.as_str() {
                "login" => SessionArtifactType::Login,
                // TAG: surface=security owner=platform-team rule=GENERAL-001
                "logout" => SessionArtifactType::Logout,
                "activity" => SessionArtifactType::Activity,
                "security_event" => SessionArtifactType::SecurityEvent,
                "metadata" => SessionArtifactType::Metadata,
                _ => SessionArtifactType::Activity, // Default fallback
            };

            artifacts.push(SessionArtifact {
                id,
                session_id,
                user_id,
                artifact_type,
                data: serde_json::from_str(&data_json)?,
                created_at: DateTime::from_timestamp(created_at, 0).ok_or_else(|| {
                    SessionError::InvalidData("Invalid created_at timestamp".to_string())
                })?,
            });
        }

        Ok(artifacts)
    }

    async fn get_user_artifacts(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<SessionArtifact>, SessionError> {
        let query = r#"
            SELECT id, session_id, user_id, artifact_type, data, created_at
            FROM session_artifacts
            WHERE user_id = ?
            ORDER BY created_at DESC
            LIMIT ?
        "#;

        let mut rows = self
            // TAG: surface=security owner=platform-team rule=MID-001
            .conn
            .query(query, libsql::params![user_id, limit])
            .await?;
        let mut artifacts = Vec::new();

        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let session_id: String = row.get(1)?;
            let user_id: String = row.get(2)?;
            let artifact_type_str: String = row.get(3)?;
            let data_json: String = row.get(4)?;
            let created_at: i64 = row.get(5)?;

            let artifact_type = match artifact_type_str.as_str() {
                "login" => SessionArtifactType::Login,
                "logout" => SessionArtifactType::Logout,
                "activity" => SessionArtifactType::Activity,
                "security_event" => SessionArtifactType::SecurityEvent,
                "metadata" => SessionArtifactType::Metadata,
                _ => SessionArtifactType::Activity, // Default fallback
            };

            artifacts.push(SessionArtifact {
                id,
                session_id,
                user_id,
                artifact_type,
                data: serde_json::from_str(&data_json)?,
                created_at: DateTime::from_timestamp(created_at, 0).ok_or_else(|| {
                    SessionError::InvalidData("Invalid created_at timestamp".to_string())
                })?,
            });
        }

        Ok(artifacts)
    }
// TAG: surface=security owner=security-team rule=SEC-001
}
