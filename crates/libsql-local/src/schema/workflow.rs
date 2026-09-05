//! Workflow schema definitions: workflows table
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! - Workflows are `BlockID` workflows (identity verification, report intake)
//! - §7.1: `BlockID` = Workflows

use anyhow::Result;
use libsql::Connection;

use super::{add_column_if_not_exists, try_create_index};
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize workflow-related tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_workflow_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing workflow tables");
    // Create workflows table for BlockID workflows.
    // Canonical columns (workflow_status, workflow_data, is_active, next_run_at)
    // match migrations/unified_schema.sql; the extra legacy columns (status,
    // raw_workflow_data, windmill fields, ...) are kept for backwards compat.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS workflows (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            workflow_type TEXT NOT NULL,
            workflow_name TEXT NOT NULL,
            workflow_description TEXT,
            workflow_status TEXT,
            workflow_data TEXT NOT NULL DEFAULT '{}',
            trigger_type TEXT NOT NULL DEFAULT 'manual',
            trigger_config TEXT,
            is_active BOOLEAN NOT NULL DEFAULT FALSE,
            status TEXT NOT NULL DEFAULT 'draft',
            last_run_at DATETIME,
            next_run_at DATETIME,
            last_run_status TEXT,
            run_count INTEGER DEFAULT 0,
            success_count INTEGER DEFAULT 0,
            failure_count INTEGER DEFAULT 0,
            windmill_script_id TEXT,
            windmill_flow_id TEXT,
            cron_schedule TEXT,
            enabled BOOLEAN DEFAULT FALSE,
            config TEXT,
            input_schema TEXT,
            output_schema TEXT,
            raw_workflow_data TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    ensure_workflow_columns(conn).await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // Create indexes for workflow tables (using defensive helper for cloud schema compatibility)
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_workflows_user_id ON workflows(user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_workflows_status ON workflows(status)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_workflows_type ON workflows(workflow_type)",
    )
    .await?;

    Ok(())
}

/// Idempotently add the canonical `workflows` columns (per
/// `migrations/unified_schema.sql`) that the legacy Rust-shaped table lacks.
///
/// Safe to run on every schema init: `ALTER TABLE ... ADD COLUMN` failures
/// (e.g. "duplicate column name") are logged and skipped.
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn ensure_workflow_columns(conn: &Connection) -> Result<()> {
    add_column_if_not_exists(conn, "workflows", "workflow_status", "TEXT").await?;
    add_column_if_not_exists(
        conn,
        "workflows",
        "workflow_data",
        "TEXT NOT NULL DEFAULT '{}'",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "workflows",
        "is_active",
        "BOOLEAN NOT NULL DEFAULT FALSE",
    )
    .await?;
    add_column_if_not_exists(conn, "workflows", "next_run_at", "DATETIME").await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_tables_module_exists() {
        // Module structure test - verifies the module compiles correctly
        let _ = 1 + 1; // Compile-time verification
    }

    async fn create_test_connection() -> Connection {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .unwrap();
        db.connect().unwrap()
    }

    /// Create the legacy Rust-shaped workflows table (pre-canonical schema
    /// drift: `status`/`raw_workflow_data`, no `workflow_status/workflow_data`).
    async fn create_legacy_workflows_table(conn: &Connection) {
        conn.execute(
            "CREATE TABLE workflows (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                workflow_type TEXT NOT NULL,
                workflow_name TEXT NOT NULL,
                workflow_description TEXT,
                trigger_type TEXT NOT NULL DEFAULT 'manual',
                trigger_config TEXT,
                status TEXT NOT NULL DEFAULT 'draft',
                last_run_at DATETIME,
                last_run_status TEXT,
                run_count INTEGER DEFAULT 0,
                success_count INTEGER DEFAULT 0,
                failure_count INTEGER DEFAULT 0,
                windmill_script_id TEXT,
                windmill_flow_id TEXT,
                cron_schedule TEXT,
                enabled BOOLEAN DEFAULT FALSE,
                config TEXT,
                input_schema TEXT,
                output_schema TEXT,
                raw_workflow_data TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            (),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_init_is_idempotent_and_inserts_canonical_columns() {
        let conn = create_test_connection().await;
        // Referenced by the workflows FK constraint.
        conn.execute("CREATE TABLE user_profile (id TEXT PRIMARY KEY)", ())
            .await
            .unwrap();
        conn.execute("INSERT INTO user_profile (id) VALUES ('user-1')", ())
            .await
            .unwrap();
        initialize_workflow_tables(&conn).await.unwrap();
        // Second run must not fail (CREATE IF NOT EXISTS + tolerated ALTERs).
        initialize_workflow_tables(&conn).await.unwrap();

        conn.execute(
            "INSERT INTO workflows (id, user_id, workflow_type, workflow_name, workflow_description, workflow_status, workflow_data, created_at, updated_at)
             VALUES ('wf-1', 'user-1', 'report', 'Test Workflow', 'desc', 'active', '{\"nodes\":[]}', datetime('now'), datetime('now'))
             ON CONFLICT(id) DO UPDATE SET workflow_status = excluded.workflow_status, workflow_data = excluded.workflow_data, updated_at = excluded.updated_at",
            (),
        )
        .await
        .unwrap();

        let mut rows = conn
            .query(
                "SELECT workflow_status, workflow_data, is_active, next_run_at FROM workflows WHERE id = 'wf-1'",
                (),
            )
        .await
        .unwrap();
        let row = rows.next().await.unwrap().expect("workflow row");
        assert_eq!(row.get::<String>(0).unwrap(), "active");
        assert_eq!(row.get::<String>(1).unwrap(), "{\"nodes\":[]}");
        assert!(!row.get::<bool>(2).unwrap());
        assert!(row.get::<Option<String>>(3).unwrap().is_none());
    }

    #[tokio::test]
    async fn test_init_migrates_legacy_shaped_table() {
        let conn = create_test_connection().await;
        create_legacy_workflows_table(&conn).await;

        // Legacy rows predate the canonical columns.
        conn.execute(
            "INSERT INTO workflows (id, user_id, workflow_type, workflow_name, status, raw_workflow_data)
             VALUES ('wf-legacy', 'user-1', 'report', 'Legacy', 'draft', '{}')",
            (),
        )
        .await
        .unwrap();

        // Init must converge the legacy table to the canonical shape, twice.
        initialize_workflow_tables(&conn).await.unwrap();
        initialize_workflow_tables(&conn).await.unwrap();

        conn.execute(
            "INSERT INTO workflows (id, user_id, workflow_type, workflow_name, workflow_description, workflow_status, workflow_data, created_at, updated_at)
             VALUES ('wf-new', 'user-1', 'report', 'New Workflow', 'desc', 'active', '{\"nodes\":[1]}', datetime('now'), datetime('now'))
             ON CONFLICT(id) DO UPDATE SET workflow_status = excluded.workflow_status, workflow_data = excluded.workflow_data, updated_at = excluded.updated_at",
            (),
        )
        .await
        .unwrap();

        // Legacy row survives with defaults for the new columns.
        let mut rows = conn
            .query(
                "SELECT workflow_status, workflow_data, is_active FROM workflows WHERE id = 'wf-legacy'",
                (),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("legacy row");
        assert!(row.get::<Option<String>>(0).unwrap().is_none());
        assert_eq!(row.get::<String>(1).unwrap(), "{}");
        assert!(!row.get::<bool>(2).unwrap());

        let mut rows = conn
            .query(
                "SELECT workflow_status, workflow_data FROM workflows WHERE id = 'wf-new'",
                (),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("new row");
        assert_eq!(row.get::<String>(0).unwrap(), "active");
        assert_eq!(row.get::<String>(1).unwrap(), "{\"nodes\":[1]}");
    }
}
