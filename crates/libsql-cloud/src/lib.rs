//! Cloud `LibSQL` (Turso) database operations for `FreshCredit`
//!
//! // `HARDCODED_SCHEMA`: 105 tables in `unified_schema.sql` (104 Rust + 1 browser-specific `blockchain_proofs`) (verified 2026-01-05)
//! Uses the unified 105-table schema from `migrations/unified_schema.sql`.
//! Cloud databases are per-user Turso instances with identical schema to local.

#![allow(clippy::wildcard_imports)]

use anyhow::Result;
use freshcredit_types::{Account, FinancialReport, FreshCreditResult, Transaction, UserId};
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Cloud `LibSQL` database client for Turso
#[derive(Debug)]
pub struct CloudClient {
    connection: libsql::Connection,
}

// TAG: surface=database owner=platform-team rule=DB-001
impl CloudClient {
    /// Create a new cloud client
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn new(database_url: &str, auth_token: &str) -> Result<Self> {
        info!("Creating cloud LibSQL client for Turso");

        let db = libsql::Builder::new_remote(database_url.to_string(), auth_token.to_string())
            .build()
            .await?;
        let connection = db.connect()?;

        Ok(Self { connection })
    }

    /// Initialize cloud database schema using unified schema
    /// // `HARDCODED_SCHEMA`: 105 tables (verified 2026-01-05)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing cloud database with unified schema (105 tables)");

        // Check if schema already exists by looking for key tables
        let key_tables = vec!["user_profile", "accounts", "transactions", "reports"];
        let mut schema_exists = true;

        for table in &key_tables {
            let mut rows = self
                .connection
                .query(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name = ?",
                    // TAG: surface=database owner=platform-team rule=GENERAL-001
                    libsql::params![table],
                )
                .await?;

            let next = rows.next().await?;
            if next.is_none() {
                info!("Table missing: {} - will initialize full schema", table);
                schema_exists = false;
                break;
            }
        }

        if schema_exists {
            info!("Cloud database schema already initialized");
        } else {
            info!("Creating unified schema in cloud database");
            freshcredit_libsql_local::schema::initialize_all_schema_tables_no_seed(
                &self.connection,
            )
            .await?;
        }

        // Existing databases may carry the legacy Rust-shaped workflows table
        // (status/raw_workflow_data instead of the canonical
        // workflow_status/workflow_data/is_active/next_run_at). This runs the
        // idempotent CREATE IF NOT EXISTS + column migrations so both fresh
        // and legacy databases converge on the canonical shape.
        freshcredit_libsql_local::schema::initialize_workflow_tables(&self.connection).await?;

        info!("Cloud database schema initialization completed");
        Ok(())
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Sync financial report to cloud (uses reports table - `BlockID`)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get financial report from cloud
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Sync account data to cloud
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Sync transaction data to cloud
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    // TAG: surface=database owner=platform-team rule=GENERAL-001
    pub async fn sync_user_profile(
        &self,
        profile: &freshcredit_libsql_local::UserProfile,
    ) -> FreshCreditResult<()> {
        info!("Syncing user profile {} to cloud", profile.platform_user_id);

        self.connection
            .execute(
                "INSERT OR REPLACE INTO user_profile (
                platform_user_id, azure_id, email, display_name, given_name, surname,
                tenant_id, object_id, verified_id_credential_id, verified_id_status,
                verified_id_issued_at,
                consumer_verified_id_credential_id, consumer_verified_id_status, consumer_verified_id_issued_at,
                provider_verified_id_credential_id, provider_verified_id_status, provider_verified_id_issued_at,
                created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                libsql::params![
                    profile.platform_user_id.clone(),
                    profile.azure_id.clone(),
                    profile.email.clone(),
                    profile.display_name.clone(),
                    profile.given_name.clone(),
                    profile.surname.clone(),
                    profile.tenant_id.clone(),
                    profile.object_id.clone(),
                    profile.verified_id_credential_id.clone(),
                    profile.verified_id_status.clone(),
                    profile.verified_id_issued_at.clone(),
                    profile.consumer_verified_id_credential_id.clone(),
                    profile.consumer_verified_id_status.clone(),
                    profile.consumer_verified_id_issued_at.clone(),
                    profile.provider_verified_id_credential_id.clone(),
                    profile.provider_verified_id_status.clone(),
                    profile.provider_verified_id_issued_at.clone(),
                    profile.created_at.clone(),
                    profile.updated_at.clone(),
                ],
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Store user profile to cloud (full profile with all fields)
    ///
    /// ARCHITECTURE: This is used during onboarding when browser DB doesn't exist yet.
    /// After onboarding, browser DB becomes PRIMARY and syncs to per-user Turso cloud.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
                    verified_id_status, verified_id_issued_at,
                    consumer_verified_id_credential_id, consumer_verified_id_status, consumer_verified_id_issued_at,
                    provider_verified_id_credential_id, provider_verified_id_status, provider_verified_id_issued_at,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                libsql::params![
                    profile.id.clone(),
                    profile.platform_user_id.clone(),
// TAG: surface=database owner=platform-team rule=DB-001
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
                    profile.consumer_verified_id_credential_id.clone(),
                    profile.consumer_verified_id_status.clone(),
                    profile.consumer_verified_id_issued_at.clone(),
                    profile.provider_verified_id_credential_id.clone(),
                    profile.provider_verified_id_status.clone(),
                    profile.provider_verified_id_issued_at.clone(),
                    profile.created_at.clone(),
                    profile.updated_at.clone(),
                ],
            )
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get user profile from cloud
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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

        (rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?)
        .map_or(Ok(None), |row| {
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
                // TAG: surface=database owner=platform-team rule=DB-001
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
                mfa_enabled: false,
                mfa_verified_at: None,
                tenant_id: "freshcredit".to_string(),
                object_id: row.get::<String>(5).unwrap_or_default(),
                verified_id_credential_id: row.get::<String>(6).ok(),
                verified_id_status: row
                    .get::<String>(7)
                    .unwrap_or_else(|_| "pending".to_string()),
                verified_id_issued_at: row.get::<String>(8).ok(),
                consumer_verified_id_credential_id: None,
                consumer_verified_id_status: "pending".to_string(),
                consumer_verified_id_issued_at: None,
                provider_verified_id_credential_id: None,
                provider_verified_id_status: "pending".to_string(),
                provider_verified_id_issued_at: None,
                created_at: row.get::<String>(9).unwrap_or_default(),
                updated_at: row.get::<String>(10).unwrap_or_default(),
            }))
        })
    }
}

mod rest;

#[cfg(test)]
mod tests;
