//! Local LibSQL database operations for FreshCredit

use anyhow::Result;
use freshcredit_types::{CreditReport, UserId, FreshCreditResult};
use tracing::info;

/// Local LibSQL database client
pub struct LocalClient {
    connection: libsql::Connection,
}

impl LocalClient {
    /// Create a new local client
    pub async fn new(database_path: &str) -> Result<Self> {
        info!("Creating local LibSQL client at: {}", database_path);
        
        let db = libsql::Builder::new_local(database_path).build().await?;
        let connection = db.connect()?;
        
        Ok(Self { connection })
    }

    /// Initialize database schema
    pub async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing local database schema");
        
        // Create tables for local storage
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS credit_reports (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                score INTEGER,
                data TEXT NOT NULL,
                generated_at TEXT NOT NULL,
                blockchain_hash TEXT
            )",
            (),
        ).await?;

        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_type TEXT NOT NULL,
                balance REAL,
                currency TEXT NOT NULL,
                institution_name TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            (),
        ).await?;

        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                amount REAL NOT NULL,
                currency TEXT NOT NULL,
                description TEXT NOT NULL,
                category TEXT,
                date TEXT NOT NULL,
                merchant_name TEXT
            )",
            (),
        ).await?;

        Ok(())
    }

    /// Store credit report locally
    pub async fn store_credit_report(&self, report: &CreditReport) -> FreshCreditResult<()> {
        info!("Storing credit report locally for user: {}", report.user_id);
        
        let data = serde_json::to_string(report)
            .map_err(|e| freshcredit_types::FreshCreditError::InternalError(e.to_string()))?;
        
        self.connection.execute(
            "INSERT OR REPLACE INTO credit_reports (id, user_id, score, data, generated_at, blockchain_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            libsql::params![
                report.id.to_string(),
                report.user_id.clone(),
                report.score,
                data,
                report.generated_at.to_rfc3339(),
                report.blockchain_hash.clone(),
            ],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }

    /// Retrieve credit report from local storage using raw SQL
    pub async fn get_credit_report(&self, user_id: &UserId) -> FreshCreditResult<Option<CreditReport>> {
        info!("Retrieving credit report from local storage for user: {}", user_id);

        let mut rows = self.connection.query(
            "SELECT data FROM credit_reports WHERE user_id = ? ORDER BY generated_at DESC LIMIT 1",
            libsql::params![user_id.clone()],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows.next().await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))? {
            let data: String = row.get(0)
                .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;
            let report: CreditReport = serde_json::from_str(&data)
                .map_err(|e| freshcredit_types::FreshCreditError::InternalError(e.to_string()))?;
            Ok(Some(report))
        } else {
            Ok(None)
        }
    }
}
