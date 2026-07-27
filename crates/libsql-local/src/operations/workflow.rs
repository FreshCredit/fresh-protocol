//! Workflow and Scoring Model database operations
//!
//! `BlockID` Workflows and `BlockScore` Models:
//! - Workflow operations (save, get, delete, publish)
//! - Scoring model operations (provider-defined models)
//!
//! COMPLIANCE: §3 - Scoring logic is owned and defined by the provider, not `FreshCredit`
//! COMPLIANCE: §4 workflow templates require provider customization

// TAG: surface=database owner=platform-team rule=DB-001
use anyhow::Result;
use tracing::info;

use crate::helpers::delete_with_tombstone;
use crate::{LocalClient, ScoringModelRecord, WorkflowRecord};

macro_rules! workflow_sql {
    ($where:literal) => {
        concat!(
            "SELECT id, user_id, workflow_type, workflow_name, workflow_description,",
            " workflow_status, workflow_data, trigger_type, trigger_config,",
            " is_active, last_run_at, next_run_at, run_count, created_at, updated_at",
            " FROM workflows ",
            $where
        )
    };
}

fn map_workflow_row(row: &libsql::Row) -> Result<WorkflowRecord> {
    Ok(WorkflowRecord {
        id: row.get(0)?,
        user_id: row.get(1)?,
        workflow_type: row.get(2)?,
        workflow_name: row.get(3)?,
        workflow_description: row.get(4)?,
        workflow_status: row.get(5)?,
        workflow_data: row.get(6)?,
        trigger_type: row.get(7)?,
        trigger_config: row.get(8)?,
        is_active: row.get::<i64>(9)? != 0,
        last_run_at: row.get(10)?,
        next_run_at: row.get(11)?,
        run_count: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

// TAG: surface=database owner=platform-team rule=DB-001
impl LocalClient {
    // ========================================================================
    // Scoring Model Operations (BlockScore)
    // NOTE: FreshCredit does NOT generate scores - providers define their own models
    // ========================================================================

    /// Save a provider-defined scoring model
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn save_scoring_model(&self, model: &ScoringModelRecord) -> Result<()> {
        info!(
            "Saving scoring model: {} for provider: {}",
            model.id, model.provider_id
        );

        let now = chrono::Utc::now().to_rfc3339();
        self.connection
            .execute(
                "INSERT OR REPLACE INTO scores (
                id, user_id, provider_id, score_model_id, score_model_name,
                score_model_version, data_elements_used, data_element_weights,
                raw_score_data, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                libsql::params![
                    model.id.clone(),
                    model.user_id.clone(),
                    model.provider_id.clone(),
                    model.score_model_id.clone(),
                    model.score_model_name.clone().unwrap_or_default(),
                    model.score_model_version.clone().unwrap_or_default(),
                    model.data_elements_used.clone().unwrap_or_default(),
                    model.data_element_weights.clone().unwrap_or_default(),
                    model.raw_score_data.clone(),
                    now.clone(),
                    now
                ],
            )
            .await?;

        Ok(())
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get scoring models for a provider
    /// P0-PERF: Limited to 1000 models to prevent memory exhaustion
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_scoring_models(&self, provider_id: &str) -> Result<Vec<ScoringModelRecord>> {
        info!("Getting scoring models for provider: {}", provider_id);

        let mut rows = self
            .connection
            .query(
                "SELECT id, user_id, provider_id, score_model_id, score_model_name,
                    score_model_version, data_elements_used, data_element_weights,
                    raw_score_data, created_at, updated_at
             FROM scores WHERE provider_id = ? ORDER BY updated_at DESC LIMIT 1000",
                libsql::params![provider_id],
            )
            .await?;

        let mut models = Vec::new();
        while let Some(row) = rows.next().await? {
            models.push(ScoringModelRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                provider_id: row.get(2)?,
                score_model_id: row.get(3)?,
                score_model_name: row.get(4)?,
                score_model_version: row.get(5)?,
                data_elements_used: row.get(6)?,
                data_element_weights: row.get(7)?,
                raw_score_data: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            });
        }

        Ok(models)
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get a specific scoring model by ID
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_scoring_model(&self, model_id: &str) -> Result<Option<ScoringModelRecord>> {
        info!("Getting scoring model: {}", model_id);

        let mut rows = self
            .connection
            .query(
                "SELECT id, user_id, provider_id, score_model_id, score_model_name,
                    score_model_version, data_elements_used, data_element_weights,
                    raw_score_data, created_at, updated_at
             FROM scores WHERE id = ?",
                libsql::params![model_id],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            Ok(Some(ScoringModelRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                provider_id: row.get(2)?,
                score_model_id: row.get(3)?,
                score_model_name: row.get(4)?,
                score_model_version: row.get(5)?,
                data_elements_used: row.get(6)?,
                data_element_weights: row.get(7)?,
                raw_score_data: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            }))
        } else {
            Ok(None)
        }
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Delete a scoring model
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn delete_scoring_model(&self, model_id: &str) -> Result<bool> {
        info!("Deleting scoring model: {}", model_id);

        // Slice C1: delete + tombstone in one transaction so the deletion
        // propagates to browser/cloud copies over HTTP sync.
        let affected = delete_with_tombstone(&self.connection, "scores", model_id).await?;

        Ok(affected > 0)
    }

    // ========================================================================
    // WORKFLOW OPERATIONS (BlockID)
    // ========================================================================

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Save a workflow (flow builder)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn save_workflow(&self, workflow: &WorkflowRecord) -> Result<()> {
        info!(
            "Saving workflow: {} for user: {}",
            workflow.id, workflow.user_id
        );

        let now = chrono::Utc::now().to_rfc3339();
        self.connection
            .execute(
                "INSERT OR REPLACE INTO workflows (
                id, user_id, workflow_type, workflow_name, workflow_description,
                workflow_status, workflow_data, trigger_type, trigger_config,
                is_active, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                libsql::params![
                    workflow.id.clone(),
                    workflow.user_id.clone(),
                    workflow.workflow_type.clone(),
                    workflow.workflow_name.clone(),
                    workflow.workflow_description.clone().unwrap_or_default(),
                    workflow
                        .workflow_status
                        .clone()
                        .unwrap_or_else(|| "draft".to_string()),
                    workflow.workflow_data.clone(),
                    workflow.trigger_type.clone().unwrap_or_default(),
                    workflow.trigger_config.clone().unwrap_or_default(),
                    workflow.is_active,
                    now.clone(),
                    now
                ],
            )
            .await?;

        Ok(())
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get all workflows for a user
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_workflows(&self, user_id: &str) -> Result<Vec<WorkflowRecord>> {
        info!("Getting workflows for user: {}", user_id);

        let mut rows = self
            .connection
            .query(
                workflow_sql!("WHERE user_id = ? ORDER BY updated_at DESC"),
                libsql::params![user_id],
            )
            .await?;

        let mut workflows = Vec::new();
        while let Some(row) = rows.next().await? {
            workflows.push(map_workflow_row(&row)?);
        }

        Ok(workflows)
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get a specific workflow by ID
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_workflow(&self, workflow_id: &str) -> Result<Option<WorkflowRecord>> {
        info!("Getting workflow: {}", workflow_id);

        let mut rows = self
            .connection
            .query(workflow_sql!("WHERE id = ?"), libsql::params![workflow_id])
            .await?;

        if let Some(row) = rows.next().await? {
            Ok(Some(map_workflow_row(&row)?))
        } else {
            Ok(None)
        }
    }

    /// Delete a workflow
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn delete_workflow(&self, workflow_id: &str) -> Result<bool> {
        info!("Deleting workflow: {}", workflow_id);

        // TAG: surface=database owner=platform-team rule=GENERAL-001
        // Slice C1: delete + tombstone in one transaction so the deletion
        // propagates to browser/cloud copies over HTTP sync.
        let affected = delete_with_tombstone(&self.connection, "workflows", workflow_id).await?;

        Ok(affected > 0)
    }

    /// Publish a workflow (set status to published and `is_active` to true)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn publish_workflow(&self, workflow_id: &str) -> Result<bool> {
        info!("Publishing workflow: {}", workflow_id);

        let now = chrono::Utc::now().to_rfc3339();
        let affected = self.connection.execute(
            "UPDATE workflows SET workflow_status = 'published', is_active = 1, updated_at = ? WHERE id = ?",
            libsql::params![now, workflow_id],
        ).await?;

        Ok(affected > 0)
    }
}
