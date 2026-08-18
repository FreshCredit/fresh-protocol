//! Agent schema tables
//!
//! Tables (Active):
//! - `agent_bindings`: User-agent identity binding (user-owned)
//! - `agent_memories`: User-designated facts (user-owned)
// TAG: surface=database owner=platform-team rule=DB-001
//! - `agentfs_kv_store`: Agent state key-value store (agent-owned)
//! - `agentfs_tool_calls`: Append-only tool call audit trail (agent-owned)
//!
//! Tables (DEPRECATED - Phase 9.1 Consolidation):
//! - `agent_interactions`: DEPRECATED - replaced by `agentfs_tool_calls`
//! - `agent_audit_events`: DEPRECATED - replaced by `agentfs_tool_calls`
//!
//! `AgentFS` Integration (Phase 9.1):
//! - `agentfs_kv_store`: Key-value store for agent state (context cache, reasoning snapshots)
//! - `agentfs_tool_calls`: Append-only audit trail for tool calls
//!
//! Phase 9 Columns (Enterprise Identity - PR-P0-3):
//! - `entra_object_id`: Entra service principal object ID
//! - `verified_credential_did`: Decentralized Identifier from verified credential
//! - `last_verified_at`: Last identity verification timestamp
//! - `identity_verified`: Whether identity has been verified (0/1)
//!
//! COMPLIANCE:
//! - AGENT-003: `agentfs_tool_calls` is append-only (INSERT only)
//! - AGENT-004: All tables include `user_id` with mandatory filter
//! - AGENT-005: Tool calls store names/categories only, not message content

use super::add_column_if_not_exists;
use anyhow::Result;
use libsql::Connection;
use tracing::{debug, info, warn};

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize agent tables
/// # Errors
///
/// Returns an error if the operation fails.
#[allow(clippy::too_many_lines)]
pub async fn initialize_agent_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing agent tables");
    // =========================================================================
    // ACTIVE TABLES (User-Owned Data)
    // =========================================================================

    // Agent bindings table - permanent user-agent identity relationship
    // Phase 9: Enhanced with Entra Agent ID fields for enterprise identity management
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_bindings (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL UNIQUE,
            binding_hash TEXT NOT NULL,
            binding_status TEXT NOT NULL DEFAULT 'active',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            blockchain_anchor_hash TEXT,
            -- Phase 9: Entra Agent ID fields
            entra_object_id TEXT,
            verified_credential_did TEXT,
            last_verified_at DATETIME,
            identity_verified INTEGER DEFAULT 0,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // CHATBOT-FIX: Migrations for Phase 9 columns on existing databases
    // These columns are queried by BindingService.get_or_create_binding()
        add_column_if_not_exists(conn, "agent_bindings", "entra_object_id", "TEXT").await?;
        add_column_if_not_exists(conn, "agent_bindings", "verified_credential_did", "TEXT").await?;
        add_column_if_not_exists(conn, "agent_bindings", "last_verified_at", "DATETIME").await?;
        add_column_if_not_exists(conn, "agent_bindings", "identity_verified", "INTEGER DEFAULT 0").await?;
        add_column_if_not_exists(conn, "agent_bindings", "blockchain_anchor_hash", "TEXT").await?;
        add_column_if_not_exists(conn, "agent_bindings", "binding_status", "TEXT NOT NULL DEFAULT 'active'").await?;

    // PR-P0-3: Validate schema after migrations
    validate_agent_bindings_schema(conn).await;

    // Agent memories table - user-designated facts only (explicit "remember" requests)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_memories (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            memory_text TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            source_session_id TEXT,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // =========================================================================
    // AGENTFS TABLES (Agent-Owned Data) - Phase 9.1
    // =========================================================================

    // AgentFS Key-Value Store - agent state and context cache
    // COMPLIANCE: AGENT-004 - user_id required for all queries
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agentfs_kv_store (
            user_id TEXT NOT NULL,
            key TEXT NOT NULL,
            value TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (user_id, key),
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // AgentFS Tool Calls - append-only audit trail for tool invocations
    // COMPLIANCE: AGENT-003 - INSERT only, no UPDATE/DELETE allowed
    // COMPLIANCE: AGENT-005 - No PII in input/output fields
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agentfs_tool_calls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            input TEXT,
            output TEXT,
            error TEXT,
            start_time INTEGER NOT NULL,
            end_time INTEGER NOT NULL,
            duration_ms INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // =========================================================================
    // DEPRECATED TABLES - Keep for migration, no new writes
    // These tables are replaced by agentfs_tool_calls (Phase 9.1 consolidation)
    // Will be removed after 90-day transition period
    // =========================================================================

    // DEPRECATED: Agent interactions - replaced by agentfs_tool_calls
    // Reason: agentfs_tool_calls provides structured tool-level audit trail
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_interactions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            topic_summary TEXT,
            artifact_types_accessed TEXT,
            intent_categories TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // DEPRECATED: Agent audit events - replaced by agentfs_tool_calls
    // Reason: agentfs_tool_calls includes policy_decision equivalent via error field
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_audit_events (
            id TEXT PRIMARY KEY,
            agent_binding_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            request_category TEXT,
            policy_decision TEXT,
            artifacts_accessed TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (agent_binding_id) REFERENCES agent_bindings (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize agent indexes
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_agent_indexes(conn: &Connection) -> Result<()> {
    // Active table indexes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agent_bindings_user_id ON agent_bindings(user_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agent_memories_user_id ON agent_memories(user_id)",
        (),
    )
    .await?;

    // AgentFS indexes (Phase 9.1)
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agentfs_kv_store_user_id ON agentfs_kv_store(user_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agentfs_tool_calls_user_id ON agentfs_tool_calls(user_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agentfs_tool_calls_name ON agentfs_tool_calls(name)",
        (),
    )
    .await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agentfs_tool_calls_start_time ON agentfs_tool_calls(start_time)", ()).await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // DEPRECATED table indexes (kept for migration period)
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agent_interactions_user_id ON agent_interactions(user_id)",
        (),
    )
    .await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_interactions_session_id ON agent_interactions(session_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_audit_events_binding_id ON agent_audit_events(agent_binding_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_audit_events_created_at ON agent_audit_events(created_at)", ()).await?;
    Ok(())
}

/// PR-P0-3: Validate `agent_bindings` schema has Phase 9 columns
///
/// Logs schema validation results at startup to help diagnose issues.
/// This is non-blocking - missing columns are logged as warnings but don't fail startup.
/// # Errors
///
/// Returns an error if the operation fails.
async fn validate_agent_bindings_schema(conn: &Connection) {
    debug!("Validating agent_bindings schema (Phase 9 columns)");

    // Query table info to check for Phase 9 columns
    let result = conn.query("PRAGMA table_info(agent_bindings)", ()).await;

    match result {
        Ok(mut rows) => {
            let mut columns: Vec<String> = Vec::new();
            while let Ok(Some(row)) = rows.next().await {
                if let Ok(name) = row.get::<String>(1) {
                    columns.push(name);
                }
            }

            // TAG: surface=database owner=platform-team rule=DB-001
            // Check for Phase 9 columns
            let phase9_columns = [
                "entra_object_id",
                "verified_credential_did",
                "last_verified_at",
                "identity_verified",
            ];

            let mut missing: Vec<&str> = Vec::new();
            let mut present: Vec<&str> = Vec::new();

            for col in &phase9_columns {
                if columns.iter().any(|c| c == *col) {
                    present.push(col);
                } else {
                    missing.push(col);
                }
            }

            if missing.is_empty() {
                info!(
                    columns = ?present,
                    "agent_bindings schema validated: all Phase 9 columns present"
                );
            } else {
                warn!(
                    missing = ?missing,
                    present = ?present,
                    "agent_bindings schema: some Phase 9 columns missing (will be added by migration)"
                );
            }

            debug!(all_columns = ?columns, "agent_bindings table columns");
        }
        Err(e) => {
            warn!(error = %e, "Failed to validate agent_bindings schema - table may not exist yet");
        }
    }
}

// TAG: surface=database owner=platform-team rule=DB-001
/// PR-P0-3: Public function to check if `agent_bindings` schema is valid
///
/// Returns Ok(true) if all Phase 9 columns are present, Ok(false) if some are missing,
/// or Err if the table doesn't exist or query fails.
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn check_agent_bindings_schema(conn: &Connection) -> Result<bool> {
    let mut rows = conn.query("PRAGMA table_info(agent_bindings)", ()).await?;

    let mut columns: Vec<String> = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        if let Ok(name) = row.get::<String>(1) {
            columns.push(name);
        }
    }

    let phase9_columns = [
        "entra_object_id",
        "verified_credential_did",
        "last_verified_at",
        "identity_verified",
    ];

    let all_present = phase9_columns
        .iter()
        .all(|col| columns.contains(&(*col).to_string()));
    Ok(all_present)
}
