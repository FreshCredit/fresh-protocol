//! Plaid product schema definitions
//!
//! Contains all Plaid product tables:
//! - auth: Account authentication data (routing/account numbers)
//! - identities: Identity data per account
//! - assets: Asset Reports
//! - balances: Balance history
//! - consumer_reports: Consumer Reports (credit data)
//! - employment: Employment verification
//! - enrich: Transaction enrichment
//! - income: Bank income
//! - income_verification: Payroll/employment verification
//! - investments_holdings, investments_securities, investments_transactions
//! - layer: Plaid Layer data
//! - liabilities: Credit cards, mortgages, student loans
//! - monitor: Plaid Monitor alerts
//! - recurring_transactions: Recurring payment detection
//! - signal_evaluations: Signal score evaluations
//! - statements: Bank statements
//! - transactions_sync: Transaction sync cursors
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;
use tracing::info;

/// Initialize Plaid-related tables (auth and identities)
pub async fn initialize_plaid_auth_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing plaid tables");
    // Create auth table for account authentication data
    conn.execute(
        "CREATE TABLE IF NOT EXISTS auth (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            account_id TEXT NOT NULL UNIQUE,
            account_number TEXT,
            routing_number TEXT,
            wire_routing_number TEXT,
            verification_status TEXT,
            raw_auth_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Create identities table for Plaid Identity data per account
    conn.execute(
        "CREATE TABLE IF NOT EXISTS identities (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            account_holder_names TEXT,
            account_holder_emails TEXT,
            account_holder_phones TEXT,
            account_holder_addresses TEXT,
            primary_email TEXT,
            secondary_email TEXT,
            other_email TEXT,
            primary_phone TEXT,
            home_phone TEXT,
            work_phone TEXT,
            mobile_phone TEXT,
            primary_address_street TEXT,
            primary_address_city TEXT,
            primary_address_region TEXT,
            primary_address_postal_code TEXT,
            primary_address_country TEXT,
            secondary_address_street TEXT,
            secondary_address_city TEXT,
            secondary_address_region TEXT,
            secondary_address_postal_code TEXT,
            secondary_address_country TEXT,
            is_primary_account_holder BOOLEAN DEFAULT FALSE,
            account_holder_type TEXT DEFAULT 'owner',
            raw_identity_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}

/// Initialize Plaid assets and balances tables
pub async fn initialize_plaid_assets_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing plaid tables");
    conn.execute(
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
        (),
    )
    .await?;

    conn.execute(
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
        (),
    )
    .await?;

    Ok(())
}

/// Initialize Plaid consumer reports and employment tables
pub async fn initialize_plaid_reports_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing plaid tables");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS consumer_reports (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            consumer_report_id TEXT UNIQUE,
            report_type TEXT,
            permissible_purpose TEXT,
            report_status TEXT,
            report_generation_time DATETIME,
            report_expiration_time DATETIME,
            consumer_consent_given BOOLEAN DEFAULT FALSE,
            consumer_consent_timestamp DATETIME,
            credit_score INTEGER,
            credit_score_model TEXT,
            credit_score_factors TEXT,
            tradelines_count INTEGER DEFAULT 0,
            inquiries_count INTEGER DEFAULT 0,
            public_records_count INTEGER DEFAULT 0,
            collections_count INTEGER DEFAULT 0,
            report_data TEXT,
            raw_consumer_report_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS employment (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            employment_id TEXT NOT NULL UNIQUE,
            employer_name TEXT,
            employer_address TEXT,
            employment_type TEXT,
            job_title TEXT,
            start_date DATE,
            end_date DATE,
            salary REAL,
            pay_frequency TEXT,
            currency TEXT DEFAULT 'USD',
            raw_employment_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}

/// Initialize Plaid enrich and income tables
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

/// Initialize Plaid investments tables
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

/// Initialize Plaid liabilities and layer tables
pub async fn initialize_plaid_liabilities_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS layer (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            layer_id TEXT NOT NULL UNIQUE,
            layer_type TEXT,
            layer_status TEXT,
            layer_data TEXT,
            raw_layer_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS liabilities (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            liability_type TEXT NOT NULL,
            aprs TEXT,
            is_overdue BOOLEAN,
            last_payment_amount REAL,
            last_payment_date DATE,
            last_statement_issue_date DATE,
            last_statement_balance REAL,
            minimum_payment_amount REAL,
            next_payment_due_date DATE,
            origination_date DATE,
            origination_principal_amount REAL,
            current_late_fee REAL,
            escrow_balance REAL,
            has_pmi BOOLEAN,
            has_prepayment_penalty BOOLEAN,
            interest_rate_percentage REAL,
            interest_rate_type TEXT,
            loan_term TEXT,
            loan_type_description TEXT,
            maturity_date DATE,
            next_monthly_payment REAL,
            past_due_amount REAL,
            property_address TEXT,
            ytd_interest_paid REAL,
            ytd_principal_paid REAL,
            raw_liability_data TEXT NOT NULL,
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

/// Initialize Plaid monitoring and recurring transaction tables
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

/// Initialize all Plaid tables (convenience function)
pub async fn initialize_all_plaid_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing plaid tables");
    initialize_plaid_auth_tables(conn).await?;
    initialize_plaid_assets_tables(conn).await?;
    initialize_plaid_reports_tables(conn).await?;
    initialize_plaid_income_tables(conn).await?;
    initialize_plaid_investments_tables(conn).await?;
    initialize_plaid_liabilities_tables(conn).await?;
    initialize_plaid_monitoring_tables(conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_plaid_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}
