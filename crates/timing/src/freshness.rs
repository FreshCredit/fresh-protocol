//! Data freshness validation
//!
//! This module provides utilities for validating data freshness and detecting stale data.

use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

/// Data freshness validator
#[derive(Debug, Clone)]
pub struct FreshnessValidator {
    thresholds: FreshnessThresholds,
}

/// Freshness thresholds for different data types
#[derive(Debug, Clone)]
pub struct FreshnessThresholds {
    pub plaid_transactions: Duration,
    pub plaid_accounts: Duration,
    pub linkedin_data: Duration,
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
    pub fn from_env() -> Self {
        Self {
            plaid_transactions: Duration::days(
                std::env::var("PLAID_TRANSACTION_FRESHNESS_DAYS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(90)
            ),
            plaid_accounts: Duration::days(
                std::env::var("PLAID_ACCOUNT_FRESHNESS_DAYS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(30)
            ),
            linkedin_data: Duration::days(
                std::env::var("LINKEDIN_FRESHNESS_DAYS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(7)
            ),
            reports: Duration::hours(
                std::env::var("REPORT_FRESHNESS_HOURS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(24)
            ),
        }
    }
}

/// Freshness status for a piece of data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessStatus {
    pub is_fresh: bool,
    pub last_synced_at: DateTime<Utc>,
    pub age: Duration,
    pub threshold: Duration,
    pub staleness_percentage: f64,
}

impl FreshnessStatus {
    /// Check if data is critically stale (>100% of threshold)
    pub fn is_critically_stale(&self) -> bool {
        self.staleness_percentage > 100.0
    }
    
    /// Check if data is approaching staleness (>80% of threshold)
    pub fn is_approaching_stale(&self) -> bool {
        self.staleness_percentage > 80.0 && !self.is_critically_stale()
    }
}

/// Data type for freshness checking
#[derive(Debug, Clone, Copy)]
pub enum DataType {
    PlaidTransactions,
    PlaidAccounts,
    LinkedInData,
    Reports,
}

impl FreshnessValidator {
    /// Create a new freshness validator with the given thresholds
    pub fn new(thresholds: FreshnessThresholds) -> Self {
        Self { thresholds }
    }
    
    /// Create a freshness validator with default thresholds
    pub fn default() -> Self {
        Self::new(FreshnessThresholds::default())
    }
    
    /// Check freshness of data
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
        let staleness_percentage = (age.num_seconds() as f64 / threshold.num_seconds() as f64) * 100.0;
        
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
    use super::*;

    #[test]
    fn test_freshness_validation() {
        let validator = FreshnessValidator::default();
        
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
        let validator = FreshnessValidator::default();
        
        // 21 hours old (87.5% of 24-hour threshold)
        let approaching_stale_time = Utc::now() - Duration::hours(21);
        let status = validator.check_freshness(DataType::Reports, approaching_stale_time);
        assert!(status.is_approaching_stale());
        assert!(!status.is_critically_stale());
    }
}

