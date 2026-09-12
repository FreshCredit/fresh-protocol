//! Notifications and webhook schema definitions
//!
//! Contains notification tables:
//! - `webhook_events`: Outbox pattern for webhook events
//! - notifications: In-app notifications
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize webhook and notification tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_notification_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing notifications tables");
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS webhook_events (
            id TEXT PRIMARY KEY,
            user_id TEXT,
            provider TEXT NOT NULL,
            event_type TEXT NOT NULL,
            event_id TEXT UNIQUE,
            payload TEXT NOT NULL,
            status TEXT DEFAULT 'pending',
            retry_count INTEGER DEFAULT 0,
            error_message TEXT,
            processed_at DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE SET NULL
        )",
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS notifications (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            notification_type TEXT NOT NULL,
            title TEXT NOT NULL,
            message TEXT,
            read_status INTEGER DEFAULT 0,
            action_url TEXT,
            metadata TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            expires_at DATETIME,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_notifications_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}
