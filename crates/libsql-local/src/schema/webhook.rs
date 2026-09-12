//! Webhook schema definitions: `webhook_events`, notifications
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;

use super::try_create_index;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize webhook-related tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_webhook_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing webhook tables");
    // Create webhook_events table
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
            processed_at DATETIME,
            error_message TEXT,
            retry_count INTEGER DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // Create notifications table. This initializer runs before
    // initialize_notification_tables (core group precedes business group), so
    // this definition MUST stay identical to notifications.rs — otherwise the
    // IF NOT EXISTS here wins and per-user cloud databases end up missing
    // metadata/updated_at, breaking the browser sync push.
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

    // Create indexes for webhook tables (using defensive helper for cloud schema compatibility)
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_webhook_events_user_id ON webhook_events(user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_webhook_events_status ON webhook_events(status)",
    )
    .await?;
    try_create_index(
        // TAG: surface=database owner=platform-team rule=DB-001
        conn,
        "CREATE INDEX IF NOT EXISTS idx_webhook_events_provider ON webhook_events(provider)",
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_webhook_tables_module_exists() {
        // Module structure test - verifies the module compiles correctly
        let _ = 1 + 1; // Compile-time verification
    }
}
