//! Schema validation database operations
//!
//! Operations for validating database schema integrity:
//! - `validate_schema_integrity`: Validate database schema integrity
//! - `validate_data_consistency`: Validate data consistency (internal helper)
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use tracing::info;

use crate::{LocalClient, SchemaValidationResult};

impl LocalClient {
    /// Validate database schema integrity
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn validate_schema_integrity(&self) -> Result<SchemaValidationResult> {
        info!("Validating database schema integrity");

        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        self.check_foreign_keys(&mut issues).await?;
        self.check_orphaned_transactions(&mut issues).await?;
        self.check_orphaned_accounts(&mut issues).await?;
        self.check_required_indexes(&mut warnings).await?;
        self.validate_data_consistency(&mut issues, &mut warnings)
            .await?;

        Ok(SchemaValidationResult {
            is_valid: issues.is_empty(),
            issues,
            warnings,
            checked_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Validate data consistency
    ///
    /// Note: Uses `&mut Vec<String>` for issues/warnings to allow accumulation
    /// across multiple validation methods. This pattern is consistent with
    /// other validation methods in this module.
    #[allow(clippy::ptr_arg)]
    async fn validate_data_consistency(
        &self,
        _issues: &mut Vec<String>,
        warnings: &mut Vec<String>,
    ) -> Result<()> {
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
            warnings.push(format!(
                "Found accounts with non-standard currencies: {invalid_currencies:?}"
            ));
        }

        // Check for transactions with invalid amounts
        let mut rows = self
            .connection
            .query(
                "SELECT COUNT(*) FROM transactions WHERE amount = 0 OR amount IS NULL",
                (),
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let invalid_amount_count: i64 = row.get(0)?;
            if invalid_amount_count > 0 {
                warnings.push(format!(
                    "Found {invalid_amount_count} transactions with zero or null amounts"
                ));
            }
        }

        // Check for future-dated transactions
        let mut rows = self
            .connection
            .query(
                "SELECT COUNT(*) FROM transactions WHERE date > datetime('now')",
                (),
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let future_count: i64 = row.get(0)?;
            if future_count > 0 {
                warnings.push(format!(
                    "Found {future_count} transactions with future dates"
                ));
            }
        }

        Ok(())
    }

    async fn check_foreign_keys(&self, issues: &mut Vec<String>) -> Result<()> {
        let mut rows = self.connection.query("PRAGMA foreign_keys", ()).await?;
        if let Some(row) = rows.next().await? {
            let fk_enabled: i64 = row.get(0)?;
            if fk_enabled == 0 {
                issues.push("Foreign key constraints are not enabled".to_string());
            }
        }
        Ok(())
    }

    async fn check_orphaned_transactions(&self, issues: &mut Vec<String>) -> Result<()> {
        let mut rows = self
            .connection
            .query(
                "SELECT COUNT(*) FROM transactions t LEFT JOIN accounts a ON t.account_id = a.id WHERE a.id IS NULL",
                (),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let count: i64 = row.get(0)?;
            if count > 0 {
                issues.push(format!(
                    "Found {count} orphaned transactions without valid accounts"
                ));
            }
        }
        Ok(())
    }

    async fn check_orphaned_accounts(&self, issues: &mut Vec<String>) -> Result<()> {
        let mut rows = self
            .connection
            .query(
                "SELECT COUNT(*) FROM accounts a LEFT JOIN user_profile u ON a.user_id = u.platform_user_id WHERE u.platform_user_id IS NULL",
                (),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let count: i64 = row.get(0)?;
            if count > 0 {
                issues.push(format!(
                    "Found {count} accounts without valid user profiles"
                ));
            }
        }
        Ok(())
    }

    async fn check_required_indexes(&self, warnings: &mut Vec<String>) -> Result<()> {
        const REQUIRED_INDEXES: &[&str] = &[
            "idx_accounts_user_id",
            "idx_transactions_account_id",
            "idx_transactions_date",
            "idx_user_profile_email",
        ];
        for index_name in REQUIRED_INDEXES {
            let mut rows = self
                .connection
                .query(
                    "SELECT name FROM sqlite_master WHERE type='index' AND name=?",
                    libsql::params![*index_name],
                )
                .await?;
            if rows.next().await?.is_none() {
                warnings.push(format!("Missing recommended index: {index_name}"));
            }
        }
        Ok(())
    }
}
