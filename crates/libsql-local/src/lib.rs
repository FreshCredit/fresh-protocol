//! Local LibSQL database operations for FreshCredit

use anyhow::Result;
use freshcredit_types::{CreditReport, UserId, FreshCreditResult};
use tracing::info;
use serde::{Deserialize, Serialize};

/// User profile for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub user_id: String,
    pub display_name: String,
    pub given_name: String,
    pub surname: String,
    pub email: String,
    pub job_title: String,
    pub street_address: String,
    pub city: String,
    pub state_province: String,
    pub postal_code: String,
    pub country_region: String,
    pub phone_number: String,
    pub mobile_phone: String,
    pub date_of_birth: String,
    pub ssn_last_four: String,
    pub preferred_name: String,
    pub emergency_contact_name: String,
    pub emergency_contact_phone: String,
    pub employer_name: String,
    pub employment_status: String,
    pub annual_income: String,
    pub preferred_currency: String,
    pub timezone: String,
    pub data_retention_days: i32,
    pub marketing_consent: bool,
    pub analytics_consent: bool,
    pub validation_score: f32,
    pub validation_status: String,
    pub validation_warnings: Vec<String>,
    pub validation_errors: Vec<String>,
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
                currency TEXT NOT NULL DEFAULT 'USD',
                institution_name TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (user_id) REFERENCES user_profiles(user_id)
            )",
            (),
        ).await?;

        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                amount REAL NOT NULL,
                currency TEXT NOT NULL DEFAULT 'USD',
                description TEXT NOT NULL,
                category TEXT,
                date TEXT NOT NULL,
                merchant_name TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
            )",
            (),
        ).await?;

        // Create user profiles table for comprehensive user information
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS user_profiles (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                given_name TEXT,
                surname TEXT,
                email TEXT NOT NULL,
                job_title TEXT,
                street_address TEXT,
                city TEXT,
                state_province TEXT,
                postal_code TEXT,
                country_region TEXT,
                phone_number TEXT,
                mobile_phone TEXT,
                date_of_birth TEXT,
                ssn_last_four TEXT,
                preferred_name TEXT,
                emergency_contact_name TEXT,
                emergency_contact_phone TEXT,
                employer_name TEXT,
                employment_status TEXT,
                annual_income TEXT,
                preferred_currency TEXT DEFAULT 'USD',
                timezone TEXT DEFAULT 'America/New_York',
                data_retention_days INTEGER DEFAULT 2555,
                marketing_consent BOOLEAN DEFAULT FALSE,
                analytics_consent BOOLEAN DEFAULT TRUE,
                validation_score REAL DEFAULT 0.0,
                validation_status TEXT DEFAULT 'pending',
                validation_warnings TEXT,
                validation_errors TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            (),
        ).await?;

        // Create indexes for better performance
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_accounts_user_id ON accounts(user_id)",
            (),
        ).await?;

        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_transactions_account_id ON transactions(account_id)",
            (),
        ).await?;

        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date)",
            (),
        ).await?;

        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_user_profiles_email ON user_profiles(email)",
            (),
        ).await?;

        // Enable foreign key constraints
        self.connection.execute("PRAGMA foreign_keys = ON", ()).await?;

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

    /// Store user profile in local database
    pub async fn store_user_profile(&self, profile: &UserProfile) -> Result<()> {
        info!("Storing user profile locally for user: {}", profile.user_id);

        let validation_warnings = serde_json::to_string(&profile.validation_warnings).unwrap_or_default();
        let validation_errors = serde_json::to_string(&profile.validation_errors).unwrap_or_default();

        self.connection.execute(
            "INSERT OR REPLACE INTO user_profiles (
                id, user_id, display_name, given_name, surname, email, job_title,
                street_address, city, state_province, postal_code, country_region,
                phone_number, mobile_phone, date_of_birth, ssn_last_four, preferred_name,
                emergency_contact_name, emergency_contact_phone, employer_name, employment_status,
                annual_income, preferred_currency, timezone, data_retention_days,
                marketing_consent, analytics_consent, validation_score, validation_status,
                validation_warnings, validation_errors, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))",
            libsql::params![
                profile.id.clone(),
                profile.user_id.clone(),
                profile.display_name.clone(),
                profile.given_name.clone(),
                profile.surname.clone(),
                profile.email.clone(),
                profile.job_title.clone(),
                profile.street_address.clone(),
                profile.city.clone(),
                profile.state_province.clone(),
                profile.postal_code.clone(),
                profile.country_region.clone(),
                profile.phone_number.clone(),
                profile.mobile_phone.clone(),
                profile.date_of_birth.clone(),
                profile.ssn_last_four.clone(),
                profile.preferred_name.clone(),
                profile.emergency_contact_name.clone(),
                profile.emergency_contact_phone.clone(),
                profile.employer_name.clone(),
                profile.employment_status.clone(),
                profile.annual_income.clone(),
                profile.preferred_currency.clone(),
                profile.timezone.clone(),
                profile.data_retention_days,
                profile.marketing_consent,
                profile.analytics_consent,
                profile.validation_score,
                profile.validation_status.clone(),
                validation_warnings,
                validation_errors,
            ],
        ).await?;

        Ok(())
    }

    /// Get user profile from local database
    pub async fn get_user_profile(&self, user_id: &str) -> Result<Option<UserProfile>> {
        info!("Retrieving user profile locally for user: {}", user_id);

        let mut rows = self.connection.query(
            "SELECT * FROM user_profiles WHERE user_id = ?",
            libsql::params![user_id],
        ).await?;

        if let Some(row) = rows.next().await? {
            let validation_warnings: Vec<String> = serde_json::from_str(
                &row.get::<String>(29).unwrap_or_default()
            ).unwrap_or_default();

            let validation_errors: Vec<String> = serde_json::from_str(
                &row.get::<String>(30).unwrap_or_default()
            ).unwrap_or_default();

            let profile = UserProfile {
                id: row.get(0)?,
                user_id: row.get(1)?,
                display_name: row.get(2)?,
                given_name: row.get(3).unwrap_or_default(),
                surname: row.get(4).unwrap_or_default(),
                email: row.get(5)?,
                job_title: row.get(6).unwrap_or_default(),
                street_address: row.get(7).unwrap_or_default(),
                city: row.get(8).unwrap_or_default(),
                state_province: row.get(9).unwrap_or_default(),
                postal_code: row.get(10).unwrap_or_default(),
                country_region: row.get(11).unwrap_or_default(),
                phone_number: row.get(12).unwrap_or_default(),
                mobile_phone: row.get(13).unwrap_or_default(),
                date_of_birth: row.get(14).unwrap_or_default(),
                ssn_last_four: row.get(15).unwrap_or_default(),
                preferred_name: row.get(16).unwrap_or_default(),
                emergency_contact_name: row.get(17).unwrap_or_default(),
                emergency_contact_phone: row.get(18).unwrap_or_default(),
                employer_name: row.get(19).unwrap_or_default(),
                employment_status: row.get(20).unwrap_or_default(),
                annual_income: row.get(21).unwrap_or_default(),
                preferred_currency: row.get(22).unwrap_or_else(|_| "USD".to_string()),
                timezone: row.get(23).unwrap_or_else(|_| "America/New_York".to_string()),
                data_retention_days: row.get(24).unwrap_or(2555),
                marketing_consent: row.get(25).unwrap_or(false),
                analytics_consent: row.get(26).unwrap_or(true),
                validation_score: row.get::<f64>(27).unwrap_or(0.0) as f32,
                validation_status: row.get(28).unwrap_or_else(|_| "pending".to_string()),
                validation_warnings,
                validation_errors,
                created_at: row.get(31).unwrap_or_default(),
                updated_at: row.get(32).unwrap_or_default(),
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
