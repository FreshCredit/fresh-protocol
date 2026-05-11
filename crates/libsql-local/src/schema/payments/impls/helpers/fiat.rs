use anyhow::Result;
use libsql::Connection;

/// Initialize fiat payment tables: customers, `funding_sources`, payments,
/// `stripe_plaid_payments`, and `virtual_accounts`.
pub async fn create_fiat_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS customers (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            stripe_customer_id TEXT UNIQUE,
            dwolla_customer_id TEXT UNIQUE,
            customer_type TEXT DEFAULT 'consumer',
            email TEXT,
            phone TEXT,
            first_name TEXT,
            last_name TEXT,
            business_name TEXT,
            business_type TEXT,
            status TEXT DEFAULT 'active',
            verification_status TEXT,
            raw_customer_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS funding_sources (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            customer_id TEXT NOT NULL,
            account_id TEXT,
            funding_source_id TEXT UNIQUE,
            funding_source_type TEXT NOT NULL,
            bank_name TEXT,
            bank_account_type TEXT,
            name TEXT,
            last_four TEXT,
            status TEXT DEFAULT 'unverified',
            verification_type TEXT,
            is_default BOOLEAN DEFAULT FALSE,
            is_active BOOLEAN DEFAULT TRUE,
            raw_funding_source_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE SET NULL
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS payments (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            customer_id TEXT NOT NULL,
            funding_source_id TEXT,
            payment_id TEXT UNIQUE,
            payment_type TEXT NOT NULL,
            amount REAL NOT NULL,
            currency TEXT DEFAULT 'USD',
            status TEXT DEFAULT 'pending',
            description TEXT,
            metadata TEXT,
            failure_reason TEXT,
            initiated_at DATETIME,
            completed_at DATETIME,
            raw_payment_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE,
            FOREIGN KEY (funding_source_id) REFERENCES funding_sources (id) ON DELETE SET NULL
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS stripe_plaid_payments (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            stripe_payment_intent_id TEXT UNIQUE,
            plaid_account_id TEXT NOT NULL,
            amount REAL NOT NULL,
            currency TEXT DEFAULT 'USD',
            status TEXT DEFAULT 'pending',
            stripe_customer_id TEXT,
            stripe_payment_method_id TEXT,
            plaid_access_token TEXT,
            description TEXT,
            metadata TEXT,
            failure_reason TEXT,
            initiated_at DATETIME,
            completed_at DATETIME,
            raw_payment_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS virtual_accounts (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            customer_id TEXT NOT NULL,
            virtual_account_id TEXT UNIQUE,
            account_number TEXT,
            routing_number TEXT,
            account_type TEXT DEFAULT 'checking',
            status TEXT DEFAULT 'active',
            balance REAL DEFAULT 0.0,
            currency TEXT DEFAULT 'USD',
            raw_virtual_account_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}
