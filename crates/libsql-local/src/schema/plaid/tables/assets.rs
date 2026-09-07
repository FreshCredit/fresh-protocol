use anyhow::Result;
use libsql::Connection;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize Plaid assets and balances tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_plaid_assets_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing plaid tables");
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS assets (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            asset_report_id TEXT NOT NULL UNIQUE,
            asset_report_token TEXT NOT NULL,
            client_report_id TEXT,
            date_generated DATETIME NOT NULL,
            days_requested INTEGER NOT NULL,
            report_type TEXT DEFAULT 'FULL',
            user_info TEXT,
            raw_asset_report_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;

    // TAG: surface=database owner=data-team rule=DB-001
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS balances (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            balance_current REAL NOT NULL,
            balance_available REAL,
            balance_limit REAL,
            iso_currency_code TEXT DEFAULT 'USD',
            unofficial_currency_code TEXT,
            last_statement_issue_date DATE,
            last_statement_balance REAL,
            minimum_balance_fee REAL,
            raw_balance_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;

    Ok(())
}
