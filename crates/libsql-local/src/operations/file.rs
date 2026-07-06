//! File upload database operations
//!
//! Operations for storing and retrieving uploaded files:
//! - `save_uploaded_file`: Save an uploaded file for AI analysis
//! - `get_uploaded_file`: Get an uploaded file by ID
//! - `update_file_ai_analysis`: Update AI analysis for an uploaded file
//! - `cleanup_expired_files`: Delete expired files
//!
//! COMPLIANCE: §5 Data and Report Handling - user-owned data

use anyhow::Result;

use crate::LocalClient;
use crate::SaveUploadedFileParams;
use crate::UploadedFile;

// TAG: surface=database owner=platform-team rule=DB-001
impl LocalClient {
    /// Save an uploaded file for AI analysis
    ///
    /// Uses `SaveUploadedFileParams` struct to consolidate parameters
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn save_uploaded_file(&self, params: &SaveUploadedFileParams<'_>) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        // Files expire after 24 hours by default
        let expires_at = params.expires_at.map_or_else(
            || (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339(),
            std::string::ToString::to_string,
        );
        // Derive file_type from mime_type
        let file_type = params
            .mime_type
            .split('/')
            .next()
            .unwrap_or("unknown")
            .to_string();

        self.connection.execute(
            "INSERT INTO uploaded_files (id, user_id, filename, file_type, file_size, mime_type, file_data, text_content, conversation_id, created_at, updated_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                id.clone(),
                params.user_id,
                params.filename,
                file_type,
                params.file_size,
                params.mime_type,
                params.file_data.clone().map_or(libsql::Value::Null, libsql::Value::Blob),
                params.text_content.map(std::string::ToString::to_string),
                params.conversation_id.map(std::string::ToString::to_string),
                now.clone(),
                now,
                expires_at
            ],
        ).await?;

        Ok(id)
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get an uploaded file by ID
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_uploaded_file(&self, file_id: &str) -> Result<Option<UploadedFile>> {
        let mut rows = self.connection.query(
            "SELECT id, user_id, filename, mime_type, file_size, file_data, text_content, ai_analysis, conversation_id, created_at, expires_at
             FROM uploaded_files WHERE id = ?",
            libsql::params![file_id],
        ).await?;

        if let Some(row) = rows.next().await? {
            Ok(Some(UploadedFile {
                id: row.get(0)?,
                user_id: row.get(1)?,
                filename: row.get(2)?,
                mime_type: row.get(3)?,
                file_size: row.get(4)?,
                file_data: row.get::<Option<Vec<u8>>>(5)?,
                text_content: row.get(6)?,
                ai_analysis: row.get(7)?,
                conversation_id: row.get(8)?,
                created_at: row.get(9)?,
                expires_at: row.get(10)?,
            }))
        } else {
            Ok(None)
        }
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Update AI analysis for an uploaded file
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn update_file_ai_analysis(&self, file_id: &str, analysis: &str) -> Result<()> {
        self.connection
            .execute(
                "UPDATE uploaded_files SET ai_analysis = ? WHERE id = ?",
                libsql::params![analysis, file_id],
            )
            .await?;
        Ok(())
    }

    /// Delete expired files
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn cleanup_expired_files(&self) -> Result<u64> {
        let now = chrono::Utc::now().to_rfc3339();
        let affected = self
            .connection
            .execute(
                "DELETE FROM uploaded_files WHERE expires_at < ?",
                libsql::params![now],
            )
            .await?;
        Ok(affected)
    }
}
