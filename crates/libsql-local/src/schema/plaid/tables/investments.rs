use anyhow::Result;
use libsql::Connection;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize Plaid investments tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_plaid_investments_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS investments_holdings (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            security_id TEXT NOT NULL,
            security_type TEXT,
            asset_class TEXT,
            institution_price REAL,
            institution_price_as_of DATE,
            institution_price_datetime DATETIME,
            institution_value REAL,
            cost_basis REAL,
            quantity REAL NOT NULL,
            iso_currency_code TEXT DEFAULT 'USD',
            unofficial_currency_code TEXT,
            vested_quantity REAL,
            vested_value REAL,
            raw_holding_data TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        // TAG: surface=database owner=platform-team rule=DB-001
        "CREATE TABLE IF NOT EXISTS investments_securities (
            id TEXT PRIMARY KEY,
            security_id TEXT NOT NULL UNIQUE,
            isin TEXT,
            cusip TEXT,
            sedol TEXT,
            institution_security_id TEXT,
            institution_id TEXT,
            proxy_security_id TEXT,
            name TEXT,
            ticker_symbol TEXT,
            is_cash_equivalent BOOLEAN DEFAULT FALSE,
            type TEXT,
            close_price REAL,
            close_price_as_of DATE,
            update_datetime DATETIME,
            iso_currency_code TEXT DEFAULT 'USD',
            unofficial_currency_code TEXT,
            market_identifier_code TEXT,
            sector TEXT,
            industry TEXT,
            option_contract TEXT,
            raw_security_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    // TAG: surface=database owner=data-team rule=DB-001
    conn.execute(
        "CREATE TABLE IF NOT EXISTS investments_transactions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            security_id TEXT,
            investment_transaction_id TEXT NOT NULL UNIQUE,
            date DATE NOT NULL,
            name TEXT NOT NULL,
            quantity REAL NOT NULL,
            amount REAL NOT NULL,
            price REAL NOT NULL,
            fees REAL,
            type TEXT NOT NULL,
            subtype TEXT,
            iso_currency_code TEXT DEFAULT 'USD',
            unofficial_currency_code TEXT,
            raw_investment_transaction_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}
