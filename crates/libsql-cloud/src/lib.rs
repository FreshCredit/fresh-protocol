//! Cloud LibSQL (Turso) database operations for FreshCredit

use anyhow::Result;
use freshcredit_types::{CreditReport, UserId, FreshCreditResult, Account, Transaction};
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

    /// Initialize cloud database schema (create if missing)
    pub async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing cloud database schema");

        // Check if key production tables exist and create them if missing
        let key_tables = vec!["user_profile", "accounts", "transactions", "reports"];
        let mut missing_tables = Vec::new();

        for table in &key_tables {
            let mut rows = self.connection.query(
                &format!("SELECT name FROM sqlite_master WHERE type='table' AND name='{}'", table),
                ()
            ).await?;

            if rows.next().await?.is_some() {
                info!("Verified production table exists: {}", table);
            } else {
                info!("Production table missing: {} - will create", table);
                missing_tables.push(table);
            }
        }

        // Create missing tables using the same schema as local databases
        if !missing_tables.is_empty() {
            info!("Creating missing production tables in cloud database");
            self.create_production_schema().await?;
        }

        info!("Cloud database schema initialization completed");
        Ok(())
    }

    /// Create production schema tables in cloud database
    async fn create_production_schema(&self) -> Result<()> {
        info!("Creating production schema in cloud database");

        // Enable foreign key constraints
        self.connection.execute("PRAGMA foreign_keys = ON", ()).await?;

        // Create user_profile table first (referenced by other tables)
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS user_profile (
                id TEXT PRIMARY KEY,
                platform_user_id TEXT NOT NULL,
                azure_id TEXT NOT NULL,
                email TEXT NOT NULL,
                display_name TEXT NOT NULL,
                given_name TEXT,
                family_name TEXT,
                surname TEXT,
                mobile_phone TEXT,
                job_title TEXT,
                street_address TEXT,
                city TEXT,
                state_province TEXT,
                postal_code TEXT,
                country_region TEXT,
                date_of_birth TEXT,
                ssn_last_four TEXT,
                employment_status TEXT,
                annual_income INTEGER,
                role TEXT DEFAULT 'consumer',
                tenant_id TEXT NOT NULL,
                object_id TEXT NOT NULL,
                verified_id_credential_id TEXT,
                verified_id_status TEXT DEFAULT 'pending',
                verified_id_issued_at TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
            (),
        ).await?;

        // Create accounts table
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_type TEXT NOT NULL,
                balance REAL,
                currency TEXT DEFAULT 'USD',
                institution_name TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
            (),
        ).await?;

        // Create transactions table
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                amount REAL NOT NULL,
                currency TEXT DEFAULT 'USD',
                description TEXT,
                category TEXT,
                date TEXT NOT NULL,
                merchant_name TEXT,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
            )",
            (),
        ).await?;

        // Create reports table
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS reports (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                report_type TEXT NOT NULL,
                data TEXT NOT NULL,
                generated_at TEXT NOT NULL,
                blockchain_hash TEXT,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
            (),
        ).await?;

        // Create workflows table for Windmill integration
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS workflows (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                definition TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                windmill_path TEXT,
                last_synced_at INTEGER,
                sync_status TEXT DEFAULT 'pending' CHECK (sync_status IN ('pending', 'synced', 'error'))
            )",
            (),
        ).await?;

        // Create indexes for workflows table
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_workflows_updated ON workflows(updated_at DESC)",
            (),
        ).await?;

        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_workflows_sync_status ON workflows(sync_status)",
            (),
        ).await?;

        info!("Production schema created successfully in cloud database");
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

    /// Sync account data to cloud
    pub async fn sync_account(&self, account: &Account) -> FreshCreditResult<()> {
        info!("Syncing account {} to cloud for user: {}", account.id, account.user_id);

        self.connection.execute(
            "INSERT OR REPLACE INTO accounts (id, user_id, account_type, balance, currency, institution_name, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                account.id.clone(),
                account.user_id.clone(),
                format!("{:?}", account.account_type), // Convert enum to string
                account.balance,
                account.currency.clone(),
                account.institution_name.clone(),
                account.created_at.to_rfc3339(),
            ],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Sync transaction data to cloud
    pub async fn sync_transaction(&self, transaction: &Transaction) -> FreshCreditResult<()> {
        info!("Syncing transaction {} to cloud for account: {}", transaction.id, transaction.account_id);

        self.connection.execute(
            "INSERT OR REPLACE INTO transactions (id, account_id, amount, currency, description, category, date, merchant_name)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                transaction.id.clone(),
                transaction.account_id.clone(),
                transaction.amount,
                transaction.currency.clone(),
                transaction.description.clone(),
                transaction.category.clone().unwrap_or_default(),
                transaction.date.to_rfc3339(),
                transaction.merchant_name.clone().unwrap_or_default(),
            ],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Sync user profile to cloud
    pub async fn sync_user_profile(&self, profile: &freshcredit_libsql_local::UserProfile) -> FreshCreditResult<()> {
        info!("Syncing user profile {} to cloud", profile.platform_user_id);

        self.connection.execute(
            "INSERT OR REPLACE INTO user_profile (
                platform_user_id, email, display_name, given_name, surname,
                object_id, verified_id_credential_id, verified_id_status,
                verified_id_issued_at, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                profile.platform_user_id.clone(),
                profile.email.clone(),
                profile.display_name.clone(),
                profile.given_name.clone(),
                profile.surname.clone(),
                profile.object_id.clone(),
                profile.verified_id_credential_id.clone(),
                profile.verified_id_status.clone(),
                profile.verified_id_issued_at.clone(),
                profile.created_at.clone(),
                profile.updated_at.clone(),
            ],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
