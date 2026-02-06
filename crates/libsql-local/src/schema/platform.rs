//! Platform schema definitions
//!
//! Contains platform-level tables:
//! - data_approval_hashes: Blockchain hashes for data approval
//! - referrals: Referral tracking
//! - platform_metrics: Daily aggregated metrics
//! - sales_pipeline: HubSpot sales data
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;
use tracing::info;

/// Initialize platform tables
pub async fn initialize_platform_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing platform tables");
    // Data approval hashes for blockchain anchoring
    conn.execute(
        "CREATE TABLE IF NOT EXISTS data_approval_hashes (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            approval_type TEXT NOT NULL DEFAULT 'plaid_data',
            data_hash TEXT NOT NULL,
            blockchain_hash TEXT,
            blockchain_tx_id TEXT,
            blockchain_block_number INTEGER,
            item_count INTEGER NOT NULL DEFAULT 0,
            accounts_count INTEGER DEFAULT 0,
            transactions_count INTEGER DEFAULT 0,
            data_summary TEXT,
            approval_status TEXT NOT NULL DEFAULT 'pending',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            anchored_at DATETIME,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Referrals for rewards program
    conn.execute(
        "CREATE TABLE IF NOT EXISTS referrals (
            id TEXT PRIMARY KEY,
            referrer_user_id TEXT NOT NULL,
            referred_user_id TEXT,
            referral_code TEXT NOT NULL,
            status TEXT DEFAULT 'pending',
            earnings_cents INTEGER DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            converted_at DATETIME,
            FOREIGN KEY (referrer_user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Platform metrics for internal dashboard
    conn.execute(
        "CREATE TABLE IF NOT EXISTS platform_metrics (
            id TEXT PRIMARY KEY,
            metric_date DATE NOT NULL,
            total_users INTEGER DEFAULT 0,
            new_users INTEGER DEFAULT 0,
            active_providers INTEGER DEFAULT 0,
            pending_providers INTEGER DEFAULT 0,
            reports_generated INTEGER DEFAULT 0,
            reports_verified INTEGER DEFAULT 0,
            revenue_cents INTEGER DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    // Sales pipeline from HubSpot
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sales_pipeline (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            hubspot_deal_id TEXT UNIQUE,
            deal_name TEXT,
            stage TEXT NOT NULL,
            value_cents INTEGER,
            contact_email TEXT,
            company_name TEXT,
            last_activity_at DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            closed_at DATETIME,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_platform_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}

