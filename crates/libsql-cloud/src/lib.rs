//! Cloud LibSQL (Turso) database operations for FreshCredit
//!
//! Uses the unified 52-table schema from migrations/unified_schema.sql.
//! Cloud databases are per-user Turso instances with identical schema to local.

use anyhow::Result;
use freshcredit_types::{Account, CreditReport, FreshCreditResult, Transaction, UserId};
use tracing::info;

/// Unified schema SQL embedded at compile time (52 tables)
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

    /// Initialize cloud database schema using unified 52-table schema
    pub async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing cloud database with unified schema (52 tables)");

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

    /// Sync report to cloud (uses reports table - BlockID)
    pub async fn sync_credit_report(&self, report: &CreditReport) -> FreshCreditResult<()> {
        info!("Syncing report to cloud for user: {}", report.user_id);

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

    /// Get report from cloud
    pub async fn get_credit_report(
        &self,
        user_id: &UserId,
    ) -> FreshCreditResult<Option<CreditReport>> {
        info!("Retrieving report from cloud for user: {}", user_id);

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
            let report: CreditReport = serde_json::from_str(&data)
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
                role: "consumer".to_string(),
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
}
