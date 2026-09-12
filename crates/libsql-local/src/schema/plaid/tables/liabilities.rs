use anyhow::Result;
use libsql::Connection;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize Plaid liabilities and layer tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_plaid_liabilities_tables(conn: &Connection) -> Result<()> {
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
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
    )
    .await?;

    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
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
    )
    .await?;

    Ok(())
}
