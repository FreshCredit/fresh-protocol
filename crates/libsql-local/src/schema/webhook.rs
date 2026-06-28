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
    conn.execute(
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
        (),
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // Create notifications table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS notifications (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            notification_type TEXT NOT NULL,
            title TEXT NOT NULL,
            message TEXT,
            action_url TEXT,
            read_status BOOLEAN DEFAULT FALSE,
            read_at DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            expires_at DATETIME,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
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
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_notifications_user_id ON notifications(user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_notifications_read_status ON notifications(read_status)",
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
