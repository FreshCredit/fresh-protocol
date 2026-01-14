//! Agent schema tables
//!
//! Tables: agent_bindings, agent_memories, agent_interactions, agent_audit_events

use anyhow::Result;
use libsql::Connection;

/// Initialize agent tables
pub async fn initialize_agent_tables(conn: &Connection) -> Result<()> {
    // Agent bindings table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_bindings (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL UNIQUE,
            binding_hash TEXT NOT NULL,
            binding_status TEXT NOT NULL DEFAULT 'active',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            blockchain_anchor_hash TEXT,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    ).await?;

    // Agent memories table (user-designated facts only)
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

    // Agent interactions (summarized, not transcripts)
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

    // Agent audit events (no PII)
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
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_bindings_user_id ON agent_bindings(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_memories_user_id ON agent_memories(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_interactions_user_id ON agent_interactions(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_interactions_session_id ON agent_interactions(session_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_audit_events_binding_id ON agent_audit_events(agent_binding_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agent_audit_events_created_at ON agent_audit_events(created_at)", ()).await?;
    Ok(())
}
