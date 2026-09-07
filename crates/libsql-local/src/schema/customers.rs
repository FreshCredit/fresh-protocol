//! Customer management schema tables
//! - `customer_activities`: Activity timeline for customers
//! - `customer_segments`: Customer segment definitions
//! - `customer_segment_memberships`: Customer to segment mappings
//! - `customer_communications`: Communication history with customers

use anyhow::Result;
use libsql::Connection;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize customer management tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_customer_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing customers tables");
    // Customer activities table - tracks all customer interactions
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS customer_activities (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            customer_id TEXT NOT NULL,
            activity_type TEXT NOT NULL,
            activity_description TEXT NOT NULL,
            related_entity_type TEXT,
            related_entity_id TEXT,
            metadata TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE
        )",
    )
    .await?;

    // Customer segments table - defines segments for grouping customers
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS customer_segments (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            color TEXT DEFAULT '#6b7280',
            criteria TEXT,
            is_dynamic INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .await?;

    // Customer segment memberships - maps customers to segments
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS customer_segment_memberships (
            id TEXT PRIMARY KEY,
            segment_id TEXT NOT NULL,
            customer_id TEXT NOT NULL,
            added_at TEXT NOT NULL DEFAULT (datetime('now')),
            added_by TEXT,
            FOREIGN KEY (segment_id) REFERENCES customer_segments (id) ON DELETE CASCADE,
            FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE,
            UNIQUE (segment_id, customer_id)
        )",
    )
    .await?;

    // Customer communications table - tracks all communications with customers
    conn.execute(
        // TAG: surface=database owner=platform-team rule=DB-001
        "CREATE TABLE IF NOT EXISTS customer_communications (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            customer_id TEXT NOT NULL,
            channel TEXT NOT NULL,
            direction TEXT NOT NULL,
            subject TEXT,
            content_preview TEXT,
            content_full TEXT,
            sent_at TEXT NOT NULL DEFAULT (datetime('now')),
            delivered_at TEXT,
            read_at TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            metadata TEXT,
            related_entity_type TEXT,
            related_entity_id TEXT,
            FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize indexes for customer tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_customer_indexes(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing customers tables");
    // Activity indexes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_customer_activities_customer_id ON customer_activities(customer_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_customer_activities_provider_id ON customer_activities(provider_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_customer_activities_created_at ON customer_activities(created_at DESC)",
        (),
    )
    .await?;

    // Segment indexes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_customer_segments_provider_id ON customer_segments(provider_id)",
        (),
    )
    .await?;

    // Membership indexes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_customer_segment_memberships_segment_id ON customer_segment_memberships(segment_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_customer_segment_memberships_customer_id ON customer_segment_memberships(customer_id)",
        (),
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // Communications indexes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_customer_communications_customer_id ON customer_communications(customer_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_customer_communications_provider_id ON customer_communications(provider_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_customer_communications_sent_at ON customer_communications(sent_at DESC)",
        (),
    )
    .await?;

    Ok(())
}
