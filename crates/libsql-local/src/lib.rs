//! Local LibSQL database operations for FreshCredit

use anyhow::Result;
use freshcredit_types::{CreditReport, UserId, FreshCreditResult};
use tracing::info;
use serde::{Deserialize, Serialize};

/// User profile for database storage (matches production schema)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub platform_user_id: String,
    pub azure_id: String,
    pub email: String,
    pub display_name: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub surname: Option<String>,
    pub mobile_phone: Option<String>,
    pub job_title: Option<String>,
    pub street_address: Option<String>,
    pub city: Option<String>,
    pub state_province: Option<String>,
    pub postal_code: Option<String>,
    pub country_region: Option<String>,
    pub date_of_birth: Option<String>,
    pub ssn_last_four: Option<String>,
    pub employment_status: Option<String>,
    pub annual_income: Option<i32>,
    pub role: String,
    pub tenant_id: String,
    pub object_id: String,
    pub verified_id_credential_id: Option<String>,
    pub verified_id_status: String,
    pub verified_id_issued_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Schema validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationResult {
    pub is_valid: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub checked_at: String,
}

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

    /// Get access to the underlying connection for direct queries
    pub fn connection(&self) -> &libsql::Connection {
        &self.connection
    }

    /// Initialize database schema using production schema
    pub async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing local database schema with production schema");

        // Enable foreign key constraints first
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

        // Create accounts table that matches production schema
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                plaid_account_id TEXT NOT NULL,
                plaid_access_token TEXT,
                account_id TEXT UNIQUE,
                institution_id TEXT,
                account_name TEXT NOT NULL,
                account_type TEXT NOT NULL,
                account_subtype TEXT,
                balance_available REAL,
                balance_current REAL,
                balance_limit REAL,
                currency_code TEXT DEFAULT 'USD',
                is_funding_source BOOLEAN DEFAULT FALSE,
                date_opened DATE,
                credit_limit DECIMAL(12,2),
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
            (),
        ).await?;

        // Create transactions table that matches production schema
        self.connection.execute(
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
                transaction_type TEXT,
                name TEXT NOT NULL,
                merchant_name TEXT,
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
                raw_transaction_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
            )",
            (),
        ).await?;

        // Create auth table for account authentication data
        self.connection.execute(
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
        ).await?;

        // Create identities table for identity verification data
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS identities (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                name TEXT,
                email TEXT,
                phone_number TEXT,
                address_street TEXT,
                address_city TEXT,
                address_region TEXT,
                address_postal_code TEXT,
                address_country TEXT,
                raw_identity_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
            )",
            (),
        ).await?;

        // Create credit_reports table
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS credit_reports (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                score INTEGER,
                data TEXT NOT NULL,
                generated_at TEXT NOT NULL,
                blockchain_hash TEXT,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
            (),
        ).await?;

        info!("Local database schema initialization completed");
        Ok(())
    }

    /// Validate database schema integrity
    pub async fn validate_schema_integrity(&self) -> Result<SchemaValidationResult> {
        info!("Validating database schema integrity");

        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        // Check if foreign key constraints are enabled
        let mut rows = self.connection.query("PRAGMA foreign_keys", ()).await?;
        if let Some(row) = rows.next().await? {
            let fk_enabled: i64 = row.get(0)?;
            if fk_enabled == 0 {
                issues.push("Foreign key constraints are not enabled".to_string());
            }
        }

        // Check for orphaned transactions (transactions without valid accounts)
        let mut rows = self.connection.query(
            "SELECT COUNT(*) FROM transactions t
             LEFT JOIN accounts a ON t.account_id = a.id
             WHERE a.id IS NULL",
            (),
        ).await?;

        if let Some(row) = rows.next().await? {
            let orphaned_count: i64 = row.get(0)?;
            if orphaned_count > 0 {
                issues.push(format!("Found {} orphaned transactions without valid accounts", orphaned_count));
            }
        }

        // Check for accounts without valid users
        let mut rows = self.connection.query(
            "SELECT COUNT(*) FROM accounts a
             LEFT JOIN user_profiles u ON a.user_id = u.user_id
             WHERE u.user_id IS NULL",
            (),
        ).await?;

        if let Some(row) = rows.next().await? {
            let orphaned_count: i64 = row.get(0)?;
            if orphaned_count > 0 {
                issues.push(format!("Found {} accounts without valid user profiles", orphaned_count));
            }
        }

        // Check for missing required indexes
        let required_indexes = vec![
            "idx_accounts_user_id",
            "idx_transactions_account_id",
            "idx_transactions_date",
            "idx_user_profiles_email"
        ];

        for index_name in required_indexes {
            let mut rows = self.connection.query(
                "SELECT name FROM sqlite_master WHERE type='index' AND name=?",
                libsql::params![index_name],
            ).await?;

            if rows.next().await?.is_none() {
                warnings.push(format!("Missing recommended index: {}", index_name));
            }
        }

        // Check data consistency
        self.validate_data_consistency(&mut issues, &mut warnings).await?;

        let is_valid = issues.is_empty();

        Ok(SchemaValidationResult {
            is_valid,
            issues,
            warnings,
            checked_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Validate data consistency
    async fn validate_data_consistency(&self, _issues: &mut Vec<String>, warnings: &mut Vec<String>) -> Result<()> {
        // Check for invalid currency codes
        let mut rows = self.connection.query(
            "SELECT DISTINCT currency FROM accounts WHERE currency NOT IN ('USD', 'EUR', 'GBP', 'CAD', 'JPY')",
            (),
        ).await?;

        let mut invalid_currencies = Vec::new();
        while let Some(row) = rows.next().await? {
            let currency: String = row.get(0)?;
            invalid_currencies.push(currency);
        }

        if !invalid_currencies.is_empty() {
            warnings.push(format!("Found accounts with non-standard currencies: {:?}", invalid_currencies));
        }

        // Check for transactions with invalid amounts
        let mut rows = self.connection.query(
            "SELECT COUNT(*) FROM transactions WHERE amount = 0 OR amount IS NULL",
            (),
        ).await?;

        if let Some(row) = rows.next().await? {
            let invalid_amount_count: i64 = row.get(0)?;
            if invalid_amount_count > 0 {
                warnings.push(format!("Found {} transactions with zero or null amounts", invalid_amount_count));
            }
        }

        // Check for future-dated transactions
        let mut rows = self.connection.query(
            "SELECT COUNT(*) FROM transactions WHERE date > datetime('now')",
            (),
        ).await?;

        if let Some(row) = rows.next().await? {
            let future_count: i64 = row.get(0)?;
            if future_count > 0 {
                warnings.push(format!("Found {} transactions with future dates", future_count));
            }
        }

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

    /// Store user profile in local database (matches production schema)
    pub async fn store_user_profile(&self, profile: &UserProfile) -> Result<()> {
        info!("Storing user profile locally for user: {}", profile.platform_user_id);

        self.connection.execute(
            "INSERT OR REPLACE INTO user_profile (
                id, platform_user_id, azure_id, email, display_name, given_name, family_name,
                surname, mobile_phone, job_title, street_address, city, state_province,
                postal_code, country_region, date_of_birth, ssn_last_four, employment_status,
                annual_income, role, tenant_id, object_id, verified_id_credential_id,
                verified_id_status, verified_id_issued_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))",
            libsql::params![
                profile.id.clone(),
                profile.platform_user_id.clone(),
                profile.azure_id.clone(),
                profile.email.clone(),
                profile.display_name.clone(),
                profile.given_name.clone().unwrap_or_default(),
                profile.family_name.clone().unwrap_or_default(),
                profile.surname.clone().unwrap_or_default(),
                profile.mobile_phone.clone().unwrap_or_default(),
                profile.job_title.clone().unwrap_or_default(),
                profile.street_address.clone().unwrap_or_default(),
                profile.city.clone().unwrap_or_default(),
                profile.state_province.clone().unwrap_or_default(),
                profile.postal_code.clone().unwrap_or_default(),
                profile.country_region.clone().unwrap_or_default(),
                profile.date_of_birth.clone().unwrap_or_default(),
                profile.ssn_last_four.clone().unwrap_or_default(),
                profile.employment_status.clone().unwrap_or_default(),
                profile.annual_income.unwrap_or(0),
                profile.role.clone(),
                profile.tenant_id.clone(),
                profile.object_id.clone(),
                profile.verified_id_credential_id.clone().unwrap_or_default(),
                profile.verified_id_status.clone(),
                profile.verified_id_issued_at.clone().unwrap_or_default(),
            ],
        ).await?;

        Ok(())
    }

    /// Get user profile from local database (matches production schema)
    pub async fn get_user_profile(&self, platform_user_id: &str) -> Result<Option<UserProfile>> {
        info!("Retrieving user profile locally for user: {}", platform_user_id);

        let mut rows = self.connection.query(
            "SELECT * FROM user_profile WHERE platform_user_id = ?",
            libsql::params![platform_user_id],
        ).await?;

        if let Some(row) = rows.next().await? {
            let profile = UserProfile {
                id: row.get(0)?,
                platform_user_id: row.get(1)?,
                azure_id: row.get(2)?,
                email: row.get(3)?,
                display_name: row.get(4)?,
                given_name: row.get::<Option<String>>(5).unwrap_or(None),
                family_name: row.get::<Option<String>>(6).unwrap_or(None),
                surname: row.get::<Option<String>>(7).unwrap_or(None),
                mobile_phone: row.get::<Option<String>>(8).unwrap_or(None),
                job_title: row.get::<Option<String>>(9).unwrap_or(None),
                street_address: row.get::<Option<String>>(10).unwrap_or(None),
                city: row.get::<Option<String>>(11).unwrap_or(None),
                state_province: row.get::<Option<String>>(12).unwrap_or(None),
                postal_code: row.get::<Option<String>>(13).unwrap_or(None),
                country_region: row.get::<Option<String>>(14).unwrap_or(None),
                date_of_birth: row.get::<Option<String>>(15).unwrap_or(None),
                ssn_last_four: row.get::<Option<String>>(16).unwrap_or(None),
                employment_status: row.get::<Option<String>>(17).unwrap_or(None),
                annual_income: row.get::<Option<i32>>(18).unwrap_or(None),
                role: row.get(19).unwrap_or_else(|_| "consumer".to_string()),
                tenant_id: row.get(20)?,
                object_id: row.get(21)?,
                verified_id_credential_id: row.get::<Option<String>>(22).unwrap_or(None),
                verified_id_status: row.get(23).unwrap_or_else(|_| "pending".to_string()),
                verified_id_issued_at: row.get::<Option<String>>(24).unwrap_or(None),
                created_at: row.get(25).unwrap_or_default(),
                updated_at: row.get(26).unwrap_or_default(),
            };

            Ok(Some(profile))
        } else {
            Ok(None)
        }
    }

    /// Store account data in local database
    pub async fn store_account(&self, account: &freshcredit_types::Account) -> Result<()> {
        info!("Storing account locally: {}", account.id);

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
        ).await?;

        Ok(())
    }

    /// Store transaction data in local database
    pub async fn store_transaction(&self, transaction: &freshcredit_types::Transaction) -> Result<()> {
        info!("Storing transaction locally: {}", transaction.id);

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
        ).await?;

        Ok(())
    }

    /// Get all accounts for a user
    pub async fn get_user_accounts(&self, user_id: &str) -> Result<Vec<freshcredit_types::Account>> {
        info!("Retrieving accounts for user: {}", user_id);

        let mut rows = self.connection.query(
            "SELECT * FROM accounts WHERE user_id = ? ORDER BY created_at DESC",
            libsql::params![user_id],
        ).await?;

        let mut accounts = Vec::new();
        while let Some(row) = rows.next().await? {
            let account_type_str: String = row.get(2)?;
            let account_type = match account_type_str.as_str() {
                "Checking" => freshcredit_types::AccountType::Checking,
                "Savings" => freshcredit_types::AccountType::Savings,
                "Credit" => freshcredit_types::AccountType::Credit,
                "Investment" => freshcredit_types::AccountType::Investment,
                _ => freshcredit_types::AccountType::Checking, // Default fallback
            };

            let account = freshcredit_types::Account {
                id: row.get(0)?,
                user_id: row.get(1)?,
                account_type,
                balance: row.get(3)?,
                currency: row.get(4)?,
                institution_name: row.get(5)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String>(6)?)
                    .map_err(|e| anyhow::anyhow!("Failed to parse date: {}", e))?
                    .with_timezone(&chrono::Utc),
            };
            accounts.push(account);
        }

        Ok(accounts)
    }

    /// Get all transactions for a user
    pub async fn get_user_transactions(&self, user_id: &str) -> Result<Vec<freshcredit_types::Transaction>> {
        info!("Retrieving transactions for user: {}", user_id);

        let mut rows = self.connection.query(
            "SELECT t.* FROM transactions t
             JOIN accounts a ON t.account_id = a.id
             WHERE a.user_id = ?
             ORDER BY t.date DESC",
            libsql::params![user_id],
        ).await?;

        let mut transactions = Vec::new();
        while let Some(row) = rows.next().await? {
            let category = row.get::<String>(5)?;
            let merchant_name = row.get::<String>(7)?;

            let transaction = freshcredit_types::Transaction {
                id: row.get(0)?,
                account_id: row.get(1)?,
                amount: row.get(2)?,
                currency: row.get(3)?,
                description: row.get(4)?,
                category: if category.is_empty() { None } else { Some(category) },
                date: chrono::DateTime::parse_from_rfc3339(&row.get::<String>(6)?)
                    .map_err(|e| anyhow::anyhow!("Failed to parse date: {}", e))?
                    .with_timezone(&chrono::Utc),
                merchant_name: if merchant_name.is_empty() { None } else { Some(merchant_name) },
            };
            transactions.push(transaction);
        }

        Ok(transactions)
    }
}
