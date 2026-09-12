//! Ticketing system schema definitions
//!
//! Contains ticketing tables:
//! - tickets: Support tickets, disputes, inquiries
//! - `ticket_comments`: Threaded comments
//! - `ticket_assignments`: Assignee tracking
//! - `ticket_sla_events`: SLA breach events
//!
//! COMPLIANCE: §13 Ticketing System Architecture
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize ticketing system tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_ticketing_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing ticketing tables");
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS tickets (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            ticket_type TEXT NOT NULL,
            subject TEXT NOT NULL,
            description TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'open',
            priority TEXT NOT NULL DEFAULT 'medium',
            category TEXT,
            related_entity_type TEXT,
            related_entity_id TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            resolved_at DATETIME,
            sla_due_at DATETIME,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;

    conn.execute(
        // TAG: surface=database owner=platform-team rule=GENERAL-001
        "CREATE TABLE IF NOT EXISTS ticket_comments (
            id TEXT PRIMARY KEY,
            ticket_id TEXT NOT NULL,
            author_id TEXT NOT NULL,
            content TEXT NOT NULL,
            is_internal INTEGER DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (ticket_id) REFERENCES tickets (id) ON DELETE CASCADE,
            FOREIGN KEY (author_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS ticket_assignments (
            id TEXT PRIMARY KEY,
            ticket_id TEXT NOT NULL,
            assignee_id TEXT NOT NULL,
            assigned_by TEXT NOT NULL,
            assigned_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            unassigned_at DATETIME,
            FOREIGN KEY (ticket_id) REFERENCES tickets (id) ON DELETE CASCADE,
            FOREIGN KEY (assignee_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (assigned_by) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS ticket_sla_events (
            id TEXT PRIMARY KEY,
            ticket_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            occurred_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            sla_target_minutes INTEGER,
            actual_minutes INTEGER,
            FOREIGN KEY (ticket_id) REFERENCES tickets (id) ON DELETE CASCADE
        )",
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_ticketing_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}
