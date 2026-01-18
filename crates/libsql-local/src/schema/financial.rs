//! Financial schema definitions: accounts, transactions
//!
//! Tables in this module:
//! - accounts: Linked financial accounts (Plaid integration) + FreshCredit tradelines
//! - transactions: Financial transaction records
//!
//! NOTE: balances table is in plaid.rs as it's part of Plaid Balance product
//!
//! TRADELINE SUPPORT:
//! Accounts with account_type = 'loan_tradeline' or 'credit_tradeline' are
//! FreshCredit-originated tradelines from UCP credit product purchases.
//! These use the origination_* and apr/term columns for tradeline tracking.
//! Only products with product_category = 'credit' create tradelines.
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;

use super::try_create_index;

/// Initialize financial tables
///
/// Creates 2 tables: accounts, transactions
/// NOTE: balances is in plaid.rs as part of Plaid Balance product
pub async fn initialize_financial_tables(conn: &Connection) -> Result<()> {
    // Create accounts table that matches production schema
    // Foreign key disabled to allow account creation before user_profile exists
    conn.execute(
        "CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            plaid_account_id TEXT,
            plaid_access_token TEXT,
            account_id TEXT UNIQUE,
            institution_id TEXT,
            institution_name TEXT,
            institution_logo TEXT,
            account_name TEXT,
            account_type TEXT NOT NULL,
            account_subtype TEXT,
            mask TEXT,
            balance_available REAL,
            balance_current REAL,
            balance_limit REAL,
            current_balance REAL,
            available_balance REAL,
            currency TEXT DEFAULT 'USD',
            currency_code TEXT DEFAULT 'USD',
            balance REAL DEFAULT 0.0,
            is_funding_source BOOLEAN DEFAULT FALSE,
            is_active BOOLEAN DEFAULT TRUE,
            date_opened DATE,
            credit_limit DECIMAL(12,2),
            -- Tradeline columns (for FreshCredit-originated accounts)
            -- Only populated when account_type = 'loan_tradeline' or 'credit_tradeline'
            origination_date DATE,
            origination_amount REAL,
            apr REAL,
            term_months INTEGER,
            monthly_payment REAL,
            offer_id TEXT,
            product_category TEXT,
            ucp_order_id TEXT,
            payment_id TEXT,
            blockchain_hash TEXT,
            block_number INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    // Create transactions table that matches production schema
    // Uses plaid_transaction_id UNIQUE constraint for deduplication
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transactions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            plaid_transaction_id TEXT UNIQUE,
            amount REAL NOT NULL,
            iso_currency_code TEXT DEFAULT 'USD',
            unofficial_currency_code TEXT,
            category TEXT,
            subcategory TEXT,
            personal_finance_category_primary TEXT,
            personal_finance_category_detailed TEXT,
            personal_finance_category_icon TEXT,
            transaction_type TEXT,
            name TEXT NOT NULL,
            merchant_name TEXT,
            merchant_logo_url TEXT,
            pending BOOLEAN DEFAULT FALSE,
            account_owner TEXT,
            date DATE NOT NULL,
            authorized_date DATE,
            location_address TEXT,
            location_city TEXT,
            location_region TEXT,
            location_postal_code TEXT,
            location_country TEXT,
            location_lat REAL,
            location_lon REAL,
            payment_channel TEXT,
            raw_transaction_data TEXT,
            blockchain_hash TEXT,
            block_number INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (account_id) REFERENCES accounts (account_id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Create indexes for accounts and transactions tables (using defensive helper for cloud schema compatibility)
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_accounts_user_id ON accounts(user_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_transactions_account_id ON transactions(account_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date)").await?;
    // ISSUE 7 FIX: Use plaid_transaction_id UNIQUE constraint on table instead of composite index
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_transactions_plaid_txn_id ON transactions(plaid_transaction_id)").await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_financial_tables_module_exists() {
        // Module structure test - verifies the module compiles correctly
        let _ = 1 + 1; // Compile-time verification
    }
}

