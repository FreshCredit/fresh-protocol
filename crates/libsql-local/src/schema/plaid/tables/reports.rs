use anyhow::Result;
use libsql::Connection;
use tracing::info;

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
