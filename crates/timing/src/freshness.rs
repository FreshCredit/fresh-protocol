//! Data freshness validation
//!
//! This module provides utilities for validating data freshness and detecting stale data.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Data freshness validator
#[derive(Debug, Clone)]
pub struct FreshnessValidator {
    thresholds: FreshnessThresholds,
}

/// Freshness thresholds for different data types
#[derive(Debug, Clone)]
pub struct FreshnessThresholds {
    /// Plaid Transactions
    pub plaid_transactions: Duration,
    /// Plaid Accounts
    pub plaid_accounts: Duration,
    /// Linkedin Data
    pub linkedin_data: Duration,
    /// Reports
    pub reports: Duration,
}

impl Default for FreshnessThresholds {
    fn default() -> Self {
        Self {
            plaid_transactions: Duration::days(90),
            plaid_accounts: Duration::days(30),
            linkedin_data: Duration::days(7),
            reports: Duration::hours(24),
        }
    }
}

impl FreshnessThresholds {
    /// Create freshness thresholds from environment variables
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            plaid_transactions: Duration::days(
                std::env::var("PLAID_TRANSACTION_FRESHNESS_DAYS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(90),
            ),
            plaid_accounts: Duration::days(
                std::env::var("PLAID_ACCOUNT_FRESHNESS_DAYS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(30),
            ),
            linkedin_data: Duration::days(
                std::env::var("LINKEDIN_FRESHNESS_DAYS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(7),
            ),
            reports: Duration::hours(
                std::env::var("REPORT_FRESHNESS_HOURS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(24),
            ),
        }
    }
}

/// Freshness status for a piece of data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessStatus {
    /// Is Fresh
    pub is_fresh: bool,
    /// Timestamp when the last synced was created/updated
    pub last_synced_at: DateTime<Utc>,
    /// Age
    pub age: Duration,
    /// Threshold
    pub threshold: Duration,
    /// Staleness Percentage
    pub staleness_percentage: f64,
}

impl FreshnessStatus {
    /// Check if data is critically stale (>100% of threshold)
    #[must_use]
    pub fn is_critically_stale(&self) -> bool {
        self.staleness_percentage > 100.0
    }

    /// Check if data is approaching staleness (>80% of threshold)
    #[must_use]
    pub fn is_approaching_stale(&self) -> bool {
        self.staleness_percentage > 80.0 && !self.is_critically_stale()
    }
}

/// Data type for freshness checking
#[derive(Debug, Clone, Copy)]
pub enum DataType {
    /// Plaidtransactions
    PlaidTransactions,
    /// Plaidaccounts
    PlaidAccounts,
    /// Linkedindata
    LinkedInData,
    /// Reports
    Reports,
}

impl FreshnessValidator {
    /// Create a new freshness validator with the given thresholds
    #[must_use]
    pub const fn new(thresholds: FreshnessThresholds) -> Self {
        Self { thresholds }
    }

    /// Create a freshness validator with default thresholds
    #[must_use]
    pub fn with_default_thresholds() -> Self {
        Self::new(FreshnessThresholds::default())
    }

    /// Check freshness of data
    #[must_use]
    pub fn check_freshness(
        &self,
        data_type: DataType,
        last_synced_at: DateTime<Utc>,
    ) -> FreshnessStatus {
        let threshold = match data_type {
            DataType::PlaidTransactions => self.thresholds.plaid_transactions,
            DataType::PlaidAccounts => self.thresholds.plaid_accounts,
            DataType::LinkedInData => self.thresholds.linkedin_data,
            DataType::Reports => self.thresholds.reports,
        };

        let age = Utc::now() - last_synced_at;
        let is_fresh = age <= threshold;
        #[allow(clippy::cast_precision_loss)] // Acceptable for staleness percentage calculation
        let staleness_percentage =
            (age.num_seconds() as f64 / threshold.num_seconds() as f64) * 100.0;

        FreshnessStatus {
            is_fresh,
            last_synced_at,
            age,
            threshold,
            staleness_percentage,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)]
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_freshness_validation() {
        let validator = FreshnessValidator::with_default_thresholds();

        // Fresh data
        let fresh_time = Utc::now() - Duration::hours(12);
        let status = validator.check_freshness(DataType::Reports, fresh_time);
        assert!(status.is_fresh);
        assert!(!status.is_critically_stale());

        // Stale data
        let stale_time = Utc::now() - Duration::hours(48);
        let status = validator.check_freshness(DataType::Reports, stale_time);
        assert!(!status.is_fresh);
        assert!(status.is_critically_stale());
    }

    #[test]
    fn test_approaching_stale() {
        let validator = FreshnessValidator::with_default_thresholds();

        // 21 hours old (87.5% of 24-hour threshold)
        let approaching_stale_time = Utc::now() - Duration::hours(21);
        let status = validator.check_freshness(DataType::Reports, approaching_stale_time);
        assert!(status.is_approaching_stale());
        assert!(!status.is_critically_stale());
    }

    #[test]
    fn test_freshness_validator_new() {
        let thresholds = FreshnessThresholds::default();
        let validator = FreshnessValidator::new(thresholds);
        let status = validator.check_freshness(DataType::PlaidTransactions, Utc::now());
        assert!(status.is_fresh);
    }

    #[test]
    fn test_freshness_thresholds_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("PLAID_TRANSACTION_FRESHNESS_DAYS", "60");
            std::env::set_var("PLAID_ACCOUNT_FRESHNESS_DAYS", "15");
            std::env::set_var("LINKEDIN_FRESHNESS_DAYS", "3");
            std::env::set_var("REPORT_FRESHNESS_HOURS", "12");
        }

        let thresholds = FreshnessThresholds::from_env();
        assert_eq!(thresholds.plaid_transactions, Duration::days(60));
        assert_eq!(thresholds.plaid_accounts, Duration::days(15));
        assert_eq!(thresholds.linkedin_data, Duration::days(3));
        assert_eq!(thresholds.reports, Duration::hours(12));

        unsafe {
            std::env::remove_var("PLAID_TRANSACTION_FRESHNESS_DAYS");
            std::env::remove_var("PLAID_ACCOUNT_FRESHNESS_DAYS");
            std::env::remove_var("LINKEDIN_FRESHNESS_DAYS");
            std::env::remove_var("REPORT_FRESHNESS_HOURS");
        }
    }

    #[test]
    fn test_freshness_status_not_approaching() {
        let status = FreshnessStatus {
            is_fresh: true,
            last_synced_at: Utc::now(),
            age: Duration::hours(1),
            threshold: Duration::hours(24),
            staleness_percentage: 4.17,
        };
        assert!(!status.is_approaching_stale());
        assert!(!status.is_critically_stale());
    }

    #[test]
    fn test_freshness_all_data_types() {
        let validator = FreshnessValidator::with_default_thresholds();

        let now = Utc::now();
        let status = validator.check_freshness(DataType::PlaidTransactions, now);
        assert!(status.is_fresh);

        let status = validator.check_freshness(DataType::PlaidAccounts, now);
        assert!(status.is_fresh);

        let status = validator.check_freshness(DataType::LinkedInData, now);
        assert!(status.is_fresh);

        let status = validator.check_freshness(DataType::Reports, now);
        assert!(status.is_fresh);
    }

    #[test]
    fn test_freshness_thresholds_from_env_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("PLAID_TRANSACTION_FRESHNESS_DAYS");
        std::env::remove_var("PLAID_ACCOUNT_FRESHNESS_DAYS");
        std::env::remove_var("LINKEDIN_FRESHNESS_DAYS");
        std::env::remove_var("REPORT_FRESHNESS_HOURS");

        let thresholds = FreshnessThresholds::from_env();
        assert_eq!(thresholds.plaid_transactions, Duration::days(90));
        assert_eq!(thresholds.plaid_accounts, Duration::days(30));
        assert_eq!(thresholds.linkedin_data, Duration::days(7));
        assert_eq!(thresholds.reports, Duration::hours(24));
    }

    #[test]
    fn test_freshness_validator_new_custom_thresholds() {
        let thresholds = FreshnessThresholds {
            plaid_transactions: Duration::days(30),
            plaid_accounts: Duration::days(7),
            linkedin_data: Duration::days(1),
            reports: Duration::hours(12),
        };
        let validator = FreshnessValidator::new(thresholds.clone());
        let status = validator.check_freshness(DataType::Reports, Utc::now() - Duration::hours(20));
        assert!(!status.is_fresh);
    }

    #[test]
    fn test_freshness_status_exactly_100_percent() {
        let status = FreshnessStatus {
            is_fresh: false,
            last_synced_at: Utc::now(),
            age: Duration::hours(24),
            threshold: Duration::hours(24),
            staleness_percentage: 100.0,
        };
        assert!(!status.is_critically_stale());
        assert!(status.is_approaching_stale());
    }

    #[test]
    fn test_freshness_status_approaching_at_81_percent() {
        let status = FreshnessStatus {
            is_fresh: true,
            last_synced_at: Utc::now(),
            age: Duration::hours(1),
            threshold: Duration::hours(24),
            staleness_percentage: 81.0,
        };
        assert!(status.is_approaching_stale());
        assert!(!status.is_critically_stale());
    }
}
