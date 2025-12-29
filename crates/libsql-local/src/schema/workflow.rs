//! Workflow schema definitions: workflows table
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! - Workflows are BlockID workflows (identity verification, report intake)
//! - §7.1: BlockID = Workflows

use anyhow::Result;
use libsql::Connection;

/// Initialize workflow-related tables
pub async fn initialize_workflow_tables(conn: &Connection) -> Result<()> {
    // Create workflows table for BlockID workflows
    conn.execute(
        "CREATE TABLE IF NOT EXISTS workflows (
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
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Create indexes for workflow tables
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_workflows_user_id ON workflows(user_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_workflows_status ON workflows(status)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_workflows_type ON workflows(workflow_type)",
        (),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_workflow_tables_module_exists() {
        // Module structure test - verifies the module compiles correctly
        assert!(true);
    }
}

