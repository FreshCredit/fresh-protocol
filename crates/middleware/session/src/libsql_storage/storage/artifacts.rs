// TAG: surface=security owner=security-team rule=SEC-001
use crate::libsql_storage::core::LibSqlSessionStorage;
use crate::storage::{SessionArtifact, SessionArtifactType, SessionError};
use tracing::debug;
impl LibSqlSessionStorage {
    pub(crate) async fn record_artifact_impl(
        &self,
        artifact: SessionArtifact,
    ) -> Result<(), SessionError> {
        let query = r"
            INSERT INTO session_artifacts (id, session_id, user_id, artifact_type, data, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
        ";

        let artifact_type_str = match artifact.artifact_type {
            SessionArtifactType::Login => "login",
            SessionArtifactType::Logout => "logout",
            SessionArtifactType::Activity => "activity",
            SessionArtifactType::SecurityEvent => "security_event",
            SessionArtifactType::Metadata => "metadata",
        };

        let data_json = serde_json::to_string(&artifact.data)?;

        self.conn
            .execute(
                query,
                libsql::params![
                    artifact.id.clone(),
                    artifact.session_id.clone(),
                    artifact.user_id,
                    artifact_type_str,
                    data_json,
                    artifact.created_at.timestamp(),
                ],
            )
            .await?;

        debug!(
            "Recorded artifact {} for session {}",
            artifact.id, artifact.session_id
        );
        Ok(())
    }
}
