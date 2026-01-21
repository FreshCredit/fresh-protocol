//! Cloud LibSQL (Turso) database operations for FreshCredit
//!
//! // HARDCODED_SCHEMA: 105 tables in unified_schema.sql (104 Rust + 1 browser-specific blockchain_proofs) (verified 2026-01-05)
//! Uses the unified 105-table schema from migrations/unified_schema.sql.
//! Cloud databases are per-user Turso instances with identical schema to local.

use anyhow::Result;
use freshcredit_types::{Account, FinancialReport, FreshCreditResult, Transaction, UserId};
use tracing::info;

/// Unified schema SQL embedded at compile time
/// // HARDCODED_SCHEMA: 105 tables (verified 2026-01-05)
/// Source: migrations/unified_schema.sql
const UNIFIED_SCHEMA_SQL: &str = include_str!("../../../../../migrations/unified_schema.sql");

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

    /// Initialize cloud database schema using unified schema
    /// // HARDCODED_SCHEMA: 105 tables (verified 2026-01-05)
    pub async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing cloud database with unified schema (105 tables)");

        // Check if schema already exists by looking for key tables
        let key_tables = vec!["user_profile", "accounts", "transactions", "reports"];
        let mut schema_exists = true;

        for table in &key_tables {
            let mut rows = self
                .connection
                .query(
                    &format!(
                        "SELECT name FROM sqlite_master WHERE type='table' AND name='{table}'"
                    ),
                    (),
                )
                .await?;

            if rows.next().await?.is_none() {
                info!("Table missing: {} - will initialize full schema", table);
                schema_exists = false;
                break;
            }
        }

        if !schema_exists {
            info!("Creating unified schema in cloud database");
            self.execute_unified_schema().await?;
        } else {
            info!("Cloud database schema already initialized");
        }

        info!("Cloud database schema initialization completed (44 tables)");
        Ok(())
    }

    /// Execute the unified schema SQL statements
    async fn execute_unified_schema(&self) -> Result<()> {
        // Enable foreign key constraints first
        self.connection
            .execute("PRAGMA foreign_keys = ON", ())
            .await?;

        // Split schema SQL into individual statements and execute each
        // Filter out comments and empty lines
        for statement in UNIFIED_SCHEMA_SQL.split(';') {
            let trimmed = statement.trim();

            // Skip empty statements and comment-only blocks
            if trimmed.is_empty() {
                continue;
            }

            // Skip pure comment lines (lines starting with --)
            let has_sql = trimmed.lines().any(|line| {
                let line_trimmed = line.trim();
                !line_trimmed.is_empty() && !line_trimmed.starts_with("--")
            });

            if !has_sql {
                continue;
            }

            // Execute the statement
            if let Err(e) = self.connection.execute(trimmed, ()).await {
                // Log warning but continue - some statements may already exist
                tracing::warn!(
                    "Schema statement warning: {} - Statement: {}...",
                    e,
                    &trimmed.chars().take(50).collect::<String>()
                );
            }
        }

        info!("Unified schema executed in cloud database");
        Ok(())
    }

    /// Sync financial report to cloud (uses reports table - BlockID)
    pub async fn sync_financial_report(&self, report: &FinancialReport) -> FreshCreditResult<()> {
        info!(
            "Syncing financial report to cloud for user: {}",
            report.user_id
        );

        let data = serde_json::to_string(report)
            .map_err(|e| freshcredit_types::FreshCreditError::InternalError(e.to_string()))?;

        self.connection.execute(
            "INSERT OR REPLACE INTO reports (id, user_id, report_type, report_status, report_data, raw_report_data, blockchain_hash, created_at, updated_at)
             VALUES (?, ?, 'financial', 'ready', ?, ?, ?, datetime('now'), datetime('now'))",
            libsql::params![
                report.id.to_string(),
                report.user_id.clone(),
                data.clone(),
                data,
                report.blockchain_hash.clone(),
            ],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Get financial report from cloud
    pub async fn get_financial_report(
        &self,
        user_id: &UserId,
    ) -> FreshCreditResult<Option<FinancialReport>> {
        info!(
            "Retrieving financial report from cloud for user: {}",
            user_id
        );

        let mut rows = self.connection.query(
            "SELECT report_data FROM reports WHERE user_id = ? ORDER BY created_at DESC LIMIT 1",
            libsql::params![user_id.clone()],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            let data: String = row
                .get(0)
                .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;
            let report: FinancialReport = serde_json::from_str(&data)
                .map_err(|e| freshcredit_types::FreshCreditError::InternalError(e.to_string()))?;
            Ok(Some(report))
        } else {
            Ok(None)
        }
    }

    // Note: blockchain_audit_trails table removed - blockchain hashes are stored
    // directly in reports, scores, and offers tables per unified schema design.

    /// Sync account data to cloud
    pub async fn sync_account(&self, account: &Account) -> FreshCreditResult<()> {
        info!(
            "Syncing account {} to cloud for user: {}",
            account.id, account.user_id
        );

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
        info!(
            "Syncing transaction {} to cloud for account: {}",
            transaction.id, transaction.account_id
        );

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
    pub async fn sync_user_profile(
        &self,
        profile: &freshcredit_libsql_local::UserProfile,
    ) -> FreshCreditResult<()> {
        info!("Syncing user profile {} to cloud", profile.platform_user_id);

        self.connection
            .execute(
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
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Store user profile to cloud (full profile with all fields)
    ///
    /// ARCHITECTURE: This is used during onboarding when browser DB doesn't exist yet.
    /// After onboarding, browser DB becomes PRIMARY and syncs to per-user Turso cloud.
    pub async fn store_user_profile(
        &self,
        profile: &freshcredit_libsql_local::UserProfile,
    ) -> FreshCreditResult<()> {
        info!(
            "Storing user profile {} to per-user Turso cloud",
            profile.platform_user_id
        );

        self.connection
            .execute(
                "INSERT OR REPLACE INTO user_profile (
                    id, platform_user_id, azure_id, email, display_name,
                    given_name, family_name, surname, mobile_phone, job_title,
                    street_address, city, state_province, postal_code, country_region,
                    date_of_birth, ssn_last_four, employment_status, annual_income,
                    role, tenant_id, object_id, verified_id_credential_id,
                    verified_id_status, verified_id_issued_at, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                libsql::params![
                    profile.id.clone(),
                    profile.platform_user_id.clone(),
                    profile.azure_id.clone(),
                    profile.email.clone(),
                    profile.display_name.clone(),
                    profile.given_name.clone(),
                    profile.family_name.clone(),
                    profile.surname.clone(),
                    profile.mobile_phone.clone(),
                    profile.job_title.clone(),
                    profile.street_address.clone(),
                    profile.city.clone(),
                    profile.state_province.clone(),
                    profile.postal_code.clone(),
                    profile.country_region.clone(),
                    profile.date_of_birth.clone(),
                    profile.ssn_last_four.clone(),
                    profile.employment_status.clone(),
                    profile.annual_income,
                    profile.role.clone(),
                    profile.tenant_id.clone(),
                    profile.object_id.clone(),
                    profile.verified_id_credential_id.clone(),
                    profile.verified_id_status.clone(),
                    profile.verified_id_issued_at.clone(),
                    profile.created_at.clone(),
                    profile.updated_at.clone(),
                ],
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Get user profile from cloud
    pub async fn get_user_profile(
        &self,
        user_email: &str,
    ) -> FreshCreditResult<Option<freshcredit_libsql_local::UserProfile>> {
        info!("Getting user profile from cloud for: {}", user_email);

        let mut rows = self
            .connection
            .query(
                "SELECT platform_user_id, email, display_name, given_name, surname,
                    object_id, verified_id_credential_id, verified_id_status,
                    verified_id_issued_at, created_at, updated_at
             FROM user_profile WHERE email = ? OR platform_user_id = ?",
                libsql::params![user_email.to_string(), user_email.to_string()],
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            Ok(Some(freshcredit_libsql_local::UserProfile {
                id: uuid::Uuid::new_v4().to_string(),
                platform_user_id: row.get::<String>(0).unwrap_or_default(),
                azure_id: row.get::<String>(5).unwrap_or_default(),
                email: row.get::<String>(1).unwrap_or_default(),
                display_name: row.get::<String>(2).unwrap_or_default(),
                given_name: row.get::<String>(3).ok(),
                family_name: row.get::<String>(4).ok(),
                surname: row.get::<String>(4).ok(),
                mobile_phone: None,
                job_title: None,
                street_address: None,
                city: None,
                state_province: None,
                postal_code: None,
                country_region: None,
                date_of_birth: None,
                ssn_last_four: None,
                employment_status: None,
                annual_income: None,
                phone_number: None,
                preferred_name: None,
                emergency_contact_name: None,
                emergency_contact_phone: None,
                employer_name: None,
                role: "consumer".to_string(),
                is_admin: false,
                provider_onboarding_complete: false,
                tenant_id: "freshcredit".to_string(),
                object_id: row.get::<String>(5).unwrap_or_default(),
                verified_id_credential_id: row.get::<String>(6).ok(),
                verified_id_status: row
                    .get::<String>(7)
                    .unwrap_or_else(|_| "pending".to_string()),
                verified_id_issued_at: row.get::<String>(8).ok(),
                created_at: row.get::<String>(9).unwrap_or_default(),
                updated_at: row.get::<String>(10).unwrap_or_default(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get count of accounts from cloud (for sync status)
    pub async fn get_account_count(&self) -> FreshCreditResult<u64> {
        info!("Getting account count from cloud");

        let mut rows = self
            .connection
            .query("SELECT COUNT(*) FROM accounts", libsql::params![])
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            Ok(row.get::<i64>(0).unwrap_or(0) as u64)
        } else {
            Ok(0)
        }
    }

    /// Get count of transactions from cloud (for sync status)
    pub async fn get_transaction_count(&self) -> FreshCreditResult<u64> {
        info!("Getting transaction count from cloud");

        let mut rows = self
            .connection
            .query("SELECT COUNT(*) FROM transactions", libsql::params![])
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            Ok(row.get::<i64>(0).unwrap_or(0) as u64)
        } else {
            Ok(0)
        }
    }

    /// Get user preferences from cloud
    pub async fn get_user_preferences(
        &self,
        user_id: &str,
    ) -> FreshCreditResult<Option<freshcredit_libsql_local::UserPreferences>> {
        info!("Getting user preferences from cloud for: {}", user_id);

        let mut rows = self.connection.query(
            "SELECT ai_agent_enabled, ai_feedback_enabled, ai_offers_enabled, ai_lenders_enabled,
                    cloud_sync_enabled, blockchain_enabled, email_notifications_enabled,
                    kilt_did_enabled, ai_mode, COALESCE(mock_data_enabled, 0) as mock_data_enabled
             FROM user_preferences WHERE user_id = ?",
            libsql::params![user_id.to_string()],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            Ok(Some(freshcredit_libsql_local::UserPreferences {
                ai_agent_enabled: row.get::<bool>(0).ok(),
                ai_feedback_enabled: row.get::<bool>(1).ok(),
                ai_offers_enabled: row.get::<bool>(2).ok(),
                ai_lenders_enabled: row.get::<bool>(3).ok(),
                cloud_sync_enabled: row.get::<bool>(4).ok(),
                blockchain_enabled: row.get::<bool>(5).ok(),
                email_notifications_enabled: row.get::<bool>(6).ok(),
                kilt_did_enabled: row.get::<bool>(7).ok(),
                ai_mode: row.get::<String>(8).ok(),
                mock_data_enabled: row.get::<bool>(9).ok(),
                onboarding_completed: row.get::<bool>(10).ok(),
                onboarding_permanently_dismissed: row.get::<bool>(11).ok(),
                onboarding_reminder_dismissed_until: row.get::<String>(12).ok(),
                plaid_connection_skipped: row.get::<bool>(13).ok(),
                plaid_reminder_dismissed_until: row.get::<String>(14).ok(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Save user preferences to cloud
    pub async fn save_user_preferences(
        &self,
        user_id: &str,
        prefs: &freshcredit_libsql_local::UserPreferences,
    ) -> FreshCreditResult<()> {
        info!("Saving user preferences to cloud for: {}", user_id);

        self.connection.execute(
            "INSERT OR REPLACE INTO user_preferences (
                user_id, ai_agent_enabled, ai_feedback_enabled, ai_offers_enabled,
                ai_lenders_enabled, cloud_sync_enabled, blockchain_enabled,
                email_notifications_enabled, kilt_did_enabled, ai_mode, mock_data_enabled, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                user_id.to_string(),
                prefs.ai_agent_enabled.unwrap_or(false),
                prefs.ai_feedback_enabled.unwrap_or(false),
                prefs.ai_offers_enabled.unwrap_or(false),
                prefs.ai_lenders_enabled.unwrap_or(false),
                prefs.cloud_sync_enabled.unwrap_or(true),
                prefs.blockchain_enabled.unwrap_or(true),
                prefs.email_notifications_enabled.unwrap_or(true),
                prefs.kilt_did_enabled.unwrap_or(false),
                prefs.ai_mode.clone().unwrap_or_else(|| "auto".to_string()),
                prefs.mock_data_enabled.unwrap_or(false),
                chrono::Utc::now().to_rfc3339(),
            ],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Get user profile by Azure ID (object_id) from per-user Turso cloud
    ///
    /// ARCHITECTURE: Used by payments/plaid routes to read user data from per-user cloud.
    pub async fn get_user_profile_by_azure_id(
        &self,
        azure_id: &str,
    ) -> FreshCreditResult<Option<freshcredit_libsql_local::UserProfile>> {
        info!("Getting user profile from cloud by azure_id: {}", azure_id);

        let mut rows = self
            .connection
            .query(
                "SELECT id, platform_user_id, azure_id, email, display_name,
                        given_name, family_name, surname, mobile_phone, job_title,
                        street_address, city, state_province, postal_code, country_region,
                        date_of_birth, ssn_last_four, employment_status, annual_income,
                        role, tenant_id, object_id, verified_id_credential_id,
                        verified_id_status, verified_id_issued_at, created_at, updated_at
                 FROM user_profile WHERE azure_id = ? OR object_id = ?",
                libsql::params![azure_id.to_string(), azure_id.to_string()],
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            Ok(Some(freshcredit_libsql_local::UserProfile {
                id: row.get(0).unwrap_or_default(),
                platform_user_id: row.get(1).unwrap_or_default(),
                azure_id: row.get(2).unwrap_or_default(),
                email: row.get(3).unwrap_or_default(),
                display_name: row.get(4).unwrap_or_default(),
                given_name: row.get(5).ok(),
                family_name: row.get(6).ok(),
                surname: row.get(7).ok(),
                mobile_phone: row.get(8).ok(),
                job_title: row.get(9).ok(),
                street_address: row.get(10).ok(),
                city: row.get(11).ok(),
                state_province: row.get(12).ok(),
                postal_code: row.get(13).ok(),
                country_region: row.get(14).ok(),
                date_of_birth: row.get(15).ok(),
                ssn_last_four: row.get(16).ok(),
                employment_status: row.get(17).ok(),
                annual_income: row.get(18).ok(),
                // New fields added in ARCH-P2-001 (columns 19-23)
                phone_number: row.get(19).ok(),
                preferred_name: row.get(20).ok(),
                emergency_contact_name: row.get(21).ok(),
                emergency_contact_phone: row.get(22).ok(),
                employer_name: row.get(23).ok(),
                // Existing fields shifted by 5 (columns 24+)
                role: row.get(24).unwrap_or_else(|_| "consumer".to_string()),
                is_admin: row.get::<i64>(25).unwrap_or(0) != 0,
                provider_onboarding_complete: row.get::<i64>(26).unwrap_or(0) != 0,
                tenant_id: row.get(27).unwrap_or_default(),
                object_id: row.get(28).unwrap_or_default(),
                verified_id_credential_id: row.get(29).ok(),
                verified_id_status: row.get(30).unwrap_or_else(|_| "pending".to_string()),
                verified_id_issued_at: row.get(31).ok(),
                created_at: row.get(32).unwrap_or_default(),
                updated_at: row.get(33).unwrap_or_default(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get all accounts for a user from per-user Turso cloud
    ///
    /// ARCHITECTURE: Used by payments/plaid routes to read account data from per-user cloud.
    pub async fn get_user_accounts(
        &self,
        user_id: &str,
    ) -> FreshCreditResult<Vec<freshcredit_types::Account>> {
        info!("Getting accounts from cloud for user: {}", user_id);

        let mut rows = self
            .connection
            .query(
                "SELECT id, user_id, account_type, balance, currency, institution_name, created_at
                 FROM accounts WHERE user_id = ? ORDER BY created_at DESC",
                libsql::params![user_id.to_string()],
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        let mut accounts = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            let account_type_str: String = row.get(2).unwrap_or_default();
            let account_type = match account_type_str.to_lowercase().as_str() {
                "checking" => freshcredit_types::AccountType::Checking,
                "savings" => freshcredit_types::AccountType::Savings,
                "credit" => freshcredit_types::AccountType::Credit,
                "investment" => freshcredit_types::AccountType::Investment,
                "loan" => freshcredit_types::AccountType::Loan,
                _ => freshcredit_types::AccountType::Checking,
            };

            let created_at_str: String = row.get(6).unwrap_or_default();
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            accounts.push(freshcredit_types::Account {
                id: row.get(0).unwrap_or_default(),
                user_id: row.get(1).unwrap_or_default(),
                account_type,
                balance: row.get(3).ok(),
                currency: row.get(4).unwrap_or_else(|_| "USD".to_string()),
                institution_name: row.get(5).unwrap_or_default(),
                created_at,
            });
        }

        Ok(accounts)
    }

    /// Get all transactions for a user from per-user Turso cloud
    ///
    /// ARCHITECTURE: Used by payments/plaid routes to read transaction data from per-user cloud.
    pub async fn get_user_transactions(
        &self,
        user_id: &str,
    ) -> FreshCreditResult<Vec<freshcredit_types::Transaction>> {
        info!("Getting transactions from cloud for user: {}", user_id);

        let mut rows = self
            .connection
            .query(
                "SELECT t.id, t.account_id, t.amount, COALESCE(t.iso_currency_code, 'USD'),
                        t.name, t.category, t.date, t.merchant_name
                 FROM transactions t
                 JOIN accounts a ON t.account_id = a.id
                 WHERE a.user_id = ?
                 ORDER BY t.date DESC",
                libsql::params![user_id.to_string()],
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        let mut transactions = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            let category: Option<String> = row.get(5).ok();
            let merchant_name: Option<String> = row.get(7).ok();
            let date_str: String = row.get(6).unwrap_or_default();
            let date = chrono::DateTime::parse_from_rfc3339(&date_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            transactions.push(freshcredit_types::Transaction {
                id: row.get(0).unwrap_or_default(),
                account_id: row.get(1).unwrap_or_default(),
                amount: row.get(2).unwrap_or(0.0),
                currency: row.get(3).unwrap_or_else(|_| "USD".to_string()),
                description: row.get(4).unwrap_or_default(),
                category,
                date,
                merchant_name,
            });
        }

        Ok(transactions)
    }
}
