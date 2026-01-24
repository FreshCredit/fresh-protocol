//! Agent schema tables
//!
//! Tables (Active):
//! - agent_bindings: User-agent identity binding (user-owned)
//! - agent_memories: User-designated facts (user-owned)
//! - agentfs_kv_store: Agent state key-value store (agent-owned)
//! - agentfs_tool_calls: Append-only tool call audit trail (agent-owned)
//!
//! Tables (DEPRECATED - Phase 9.1 Consolidation):
//! - agent_interactions: DEPRECATED - replaced by agentfs_tool_calls
//! - agent_audit_events: DEPRECATED - replaced by agentfs_tool_calls
//!
//! AgentFS Integration (Phase 9.1):
//! - agentfs_kv_store: Key-value store for agent state (context cache, reasoning snapshots)
//! - agentfs_tool_calls: Append-only audit trail for tool calls
//!
//! COMPLIANCE:
//! - AGENT-003: agentfs_tool_calls is append-only (INSERT only)
//! - AGENT-004: All tables include user_id with mandatory filter
//! - AGENT-005: Tool calls store names/categories only, not message content

use anyhow::Result;
use libsql::Connection;

/// Initialize agent tables
pub async fn initialize_agent_tables(conn: &Connection) -> Result<()> {
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
    ).await?;

    // CHATBOT-FIX: Migrations for Phase 9 columns on existing databases
    // These columns are queried by BindingService.get_or_create_binding()
    let _ = conn.execute(
        "ALTER TABLE agent_bindings ADD COLUMN entra_object_id TEXT",
        (),
    ).await;
    let _ = conn.execute(
        "ALTER TABLE agent_bindings ADD COLUMN verified_credential_did TEXT",
        (),
    ).await;
    let _ = conn.execute(
        "ALTER TABLE agent_bindings ADD COLUMN last_verified_at DATETIME",
        (),
    ).await;
    let _ = conn.execute(
        "ALTER TABLE agent_bindings ADD COLUMN identity_verified INTEGER DEFAULT 0",
        (),
    ).await;
    let _ = conn.execute(
        "ALTER TABLE agent_bindings ADD COLUMN blockchain_anchor_hash TEXT",
        (),
    ).await;
    let _ = conn.execute(
        "ALTER TABLE agent_bindings ADD COLUMN binding_status TEXT NOT NULL DEFAULT 'active'",
        (),
    ).await;

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
    ).await?;

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
    ).await?;

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
    ).await?;

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
    ).await?;

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
    ).await?;

    Ok(())
}

/// Initialize agent indexes
pub async fn initialize_agent_indexes(conn: &Connection) -> Result<()> {
    // Active table indexes
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_bindings_user_id ON agent_bindings(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_memories_user_id ON agent_memories(user_id)", ()).await?;

    // AgentFS indexes (Phase 9.1)
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agentfs_kv_store_user_id ON agentfs_kv_store(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agentfs_tool_calls_user_id ON agentfs_tool_calls(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agentfs_tool_calls_name ON agentfs_tool_calls(name)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agentfs_tool_calls_start_time ON agentfs_tool_calls(start_time)", ()).await?;

    // DEPRECATED table indexes (kept for migration period)
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_interactions_user_id ON agent_interactions(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_interactions_session_id ON agent_interactions(session_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_audit_events_binding_id ON agent_audit_events(agent_binding_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_audit_events_created_at ON agent_audit_events(created_at)", ()).await?;
    Ok(())
}
