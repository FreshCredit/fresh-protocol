//! Financial schema definitions: accounts, transactions, balances
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;

/// Initialize financial tables
pub async fn initialize_financial_tables(conn: &Connection) -> Result<()> {
    // Accounts table for linked financial accounts
    conn.execute(
        "CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            plaid_account_id TEXT,
            institution_id TEXT,
            institution_name TEXT,
            account_name TEXT NOT NULL,
            account_type TEXT NOT NULL,
            account_subtype TEXT,
            mask TEXT,
            current_balance REAL,
            available_balance REAL,
            currency_code TEXT DEFAULT 'USD',
            is_active BOOLEAN DEFAULT TRUE,
            last_synced_at TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile(id)
        )",
        (),
    )
    .await?;

    // Transactions table for financial transactions
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transactions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            plaid_transaction_id TEXT,
            amount REAL NOT NULL,
            currency_code TEXT DEFAULT 'USD',
            date TEXT NOT NULL,
            name TEXT NOT NULL,
            merchant_name TEXT,
            category TEXT,
            category_id TEXT,
            pending BOOLEAN DEFAULT FALSE,
            payment_channel TEXT,
            location_city TEXT,
            location_state TEXT,
            location_country TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile(id),
            FOREIGN KEY (account_id) REFERENCES accounts(id)
        )",
        (),
    )
    .await?;

    // Balances table for balance history
    conn.execute(
        "CREATE TABLE IF NOT EXISTS balances (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            current_balance REAL NOT NULL,
            available_balance REAL,
            currency_code TEXT DEFAULT 'USD',
            recorded_at TEXT NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile(id),
            FOREIGN KEY (account_id) REFERENCES accounts(id)
        )",
        (),
    )
    .await?;

    // Create indexes for financial tables
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_accounts_user ON accounts(user_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_transactions_user ON transactions(user_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_transactions_account ON transactions(account_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_balances_user ON balances(user_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_balances_account ON balances(account_id)",
        (),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_financial_tables_module_exists() {
        // Module structure test - verifies the module compiles correctly
        assert!(true);
    }
}

