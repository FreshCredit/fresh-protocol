//! Cloud LibSQL (Turso) database operations for FreshCredit

use anyhow::Result;
use freshcredit_types::{CreditReport, UserId, FreshCreditResult};
use tracing::info;

/// Cloud LibSQL database client for Turso
pub struct CloudClient {
    connection: libsql::Connection,
}

impl CloudClient {
    /// Create a new cloud client
    pub async fn new(database_url: &str, auth_token: &str) -> Result<Self> {
        info!("Creating cloud LibSQL client for Turso");
        
        let db = libsql::Builder::new_remote(database_url.to_string(), auth_token.to_string())
            .build()
            .await?;
        let connection = db.connect()?;
        
        Ok(Self { connection })
    }

    /// Initialize cloud database schema
    pub async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing cloud database schema");
        
        // Create production tables in Turso
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS credit_reports (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                score INTEGER,
                data TEXT NOT NULL,
                generated_at TEXT NOT NULL,
                blockchain_hash TEXT,
                synced_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
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
                created_at TEXT NOT NULL,
                synced_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
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
                merchant_name TEXT,
                synced_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
            (),
        ).await?;

        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS blockchain_audit_trails (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                data_hash TEXT NOT NULL,
                blockchain_hash TEXT NOT NULL,
                transaction_id TEXT,
                created_at TEXT NOT NULL,
                synced_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
            (),
        ).await?;

        Ok(())
    }

    /// Sync credit report to cloud
    pub async fn sync_credit_report(&self, report: &CreditReport) -> FreshCreditResult<()> {
        info!("Syncing credit report to cloud for user: {}", report.user_id);
        
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

    /// Get credit report from cloud
    pub async fn get_credit_report(&self, user_id: &UserId) -> FreshCreditResult<Option<CreditReport>> {
        info!("Retrieving credit report from cloud for user: {}", user_id);
        
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

    /// Store blockchain audit trail
    pub async fn store_blockchain_audit(&self, user_id: &UserId, data_hash: &str, blockchain_hash: &str) -> FreshCreditResult<()> {
        info!("Storing blockchain audit trail for user: {}", user_id);
        
        self.connection.execute(
            "INSERT INTO blockchain_audit_trails (id, user_id, data_hash, blockchain_hash, created_at)
             VALUES (?, ?, ?, ?, ?)",
            libsql::params![
                uuid::Uuid::new_v4().to_string(),
                user_id.clone(),
                data_hash,
                blockchain_hash,
                chrono::Utc::now().to_rfc3339(),
            ],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;
        
        Ok(())
    }

    /// Get blockchain audit trail for user
    pub async fn get_blockchain_audit_trail(&self, user_id: &UserId) -> FreshCreditResult<Vec<serde_json::Value>> {
        info!("Getting blockchain audit trail for user: {}", user_id);
        
        let mut audit_trail = Vec::new();
        let mut rows = self.connection.query(
            "SELECT data_hash, blockchain_hash, transaction_id, created_at FROM blockchain_audit_trails
             WHERE user_id = ? ORDER BY created_at DESC",
            libsql::params![user_id.clone()],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        while let Some(row) = rows.next().await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))? {
            let data_hash: String = row.get(0)
                .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;
            let blockchain_hash: String = row.get(1)
                .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;
            let transaction_id: Option<String> = row.get(2)
                .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;
            let created_at: String = row.get(3)
                .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;
            
            audit_trail.push(serde_json::json!({
                "data_hash": data_hash,
                "blockchain_hash": blockchain_hash,
                "transaction_id": transaction_id,
                "created_at": created_at
            }));
        }
        
        Ok(audit_trail)
    }
}
