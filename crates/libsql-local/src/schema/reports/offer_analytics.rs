use anyhow::Result;
use libsql::Connection;

/// Initialize offer analytics tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_offer_analytics_tables(conn: &Connection) -> Result<()> {
    // Daily offer analytics metrics
    conn.execute(
        "CREATE TABLE IF NOT EXISTS offer_analytics (
            id TEXT PRIMARY KEY,
            offer_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            date TEXT NOT NULL,
            impressions INTEGER DEFAULT 0,
            views INTEGER DEFAULT 0,
            clicks INTEGER DEFAULT 0,
            applications INTEGER DEFAULT 0,
            conversions INTEGER DEFAULT 0,
            revenue_cents INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (offer_id) REFERENCES provider_offers (id) ON DELETE CASCADE,
            UNIQUE (offer_id, date)
        )",
        (),
    )
    .await?;

    // A/B test results for offer variants
    conn.execute(
        "CREATE TABLE IF NOT EXISTS offer_ab_test_results (
            id TEXT PRIMARY KEY,
            offer_id TEXT NOT NULL,
            variant TEXT NOT NULL,
            impressions INTEGER DEFAULT 0,
            conversions INTEGER DEFAULT 0,
            revenue_cents INTEGER DEFAULT 0,
            start_date TEXT NOT NULL,
            end_date TEXT,
            is_active INTEGER DEFAULT 1,
            statistical_significance REAL,
            winner INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (offer_id) REFERENCES provider_offers (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Offer lifecycle events for conversion tracking
    conn.execute(
        "CREATE TABLE IF NOT EXISTS offer_events (
            id TEXT PRIMARY KEY,
            offer_id TEXT NOT NULL,
            user_id TEXT,
            event_type TEXT NOT NULL,
            event_data TEXT,
            session_id TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (offer_id) REFERENCES provider_offers (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}
