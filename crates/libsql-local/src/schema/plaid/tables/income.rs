use anyhow::Result;
use libsql::Connection;

/// Initialize Plaid enrich and income tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_plaid_income_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS enrich (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            transaction_id TEXT NOT NULL,
            enriched_merchant_name TEXT,
            enriched_category TEXT,
            enriched_subcategory TEXT,
            merchant_logo_url TEXT,
            merchant_website TEXT,
            merchant_phone_number TEXT,
            merchant_address TEXT,
            confidence_level REAL,
            enrichment_timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            raw_enrich_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE,
            FOREIGN KEY (transaction_id) REFERENCES transactions (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS income (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            bank_income_id TEXT NOT NULL UNIQUE,
            generated_time DATETIME NOT NULL,
            days_requested INTEGER NOT NULL,
            item_id TEXT NOT NULL,
            institution_id TEXT,
            institution_name TEXT,
            raw_bank_income_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS income_verification (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            income_verification_id TEXT NOT NULL UNIQUE,
            item_id TEXT,
            employer_name TEXT,
            employee_name TEXT,
            pay_period_start DATE,
            pay_period_end DATE,
            pay_date DATE,
            gross_pay REAL,
            net_pay REAL,
            pay_frequency TEXT,
            verification_status TEXT,
            raw_income_verification_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}
