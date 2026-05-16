use anyhow::Result;
use libsql::Connection;

/// Initialize Plaid monitoring and recurring transaction tables
/// # Errors
///
/// Returns an error if the operation fails.
#[allow(clippy::too_many_lines)]
pub async fn initialize_plaid_monitoring_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS monitor (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            item_id TEXT NOT NULL,
            monitor_id TEXT NOT NULL UNIQUE,
            monitor_type TEXT,
            monitor_status TEXT,
            last_check_time DATETIME,
            next_check_time DATETIME,
            alert_settings TEXT,
            raw_monitor_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS recurring_transactions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            stream_id TEXT NOT NULL UNIQUE,
            category TEXT,
            category_id TEXT,
            description TEXT,
            merchant_name TEXT,
            personal_finance_category TEXT,
            first_date DATE,
            last_date DATE,
            frequency TEXT,
            transaction_ids TEXT,
            average_amount REAL,
            average_amount_is_estimated BOOLEAN DEFAULT FALSE,
            last_amount REAL,
            is_active BOOLEAN DEFAULT TRUE,
            status TEXT,
            is_user_modified BOOLEAN DEFAULT FALSE,
            last_user_modified_datetime DATETIME,
            raw_recurring_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS signal_evaluations (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            client_transaction_id TEXT NOT NULL UNIQUE,
            amount REAL NOT NULL,
            client_user_id TEXT,
            user_present BOOLEAN,
            device TEXT,
            scores TEXT,
            core_attributes TEXT,
            warnings TEXT,
            request_id TEXT,
            raw_signal_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS statements (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            statement_id TEXT NOT NULL UNIQUE,
            month INTEGER NOT NULL,
            year INTEGER NOT NULL,
            institution_id TEXT,
            institution_name TEXT,
            statement_pdf_url TEXT,
            statement_pdf_data BLOB,
            raw_statement_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS transactions_sync (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            item_id TEXT NOT NULL UNIQUE,
            access_token TEXT NOT NULL,
            cursor TEXT,
            has_more BOOLEAN DEFAULT FALSE,
            added_count INTEGER DEFAULT 0,
            modified_count INTEGER DEFAULT 0,
            removed_count INTEGER DEFAULT 0,
            last_sync_time DATETIME,
            raw_sync_data TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}
