//! ACH settlement timing calculations for FreshCredit
//!
//! Implements business day calculations for ACH settlement dates (T+1 to T+4).
//! Uses BusinessDayCalendar for holiday and weekend handling.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::calendar::BusinessDayCalendar;

/// ACH settlement timing calculator
pub struct SettlementCalculator {
    calendar: BusinessDayCalendar,
}

impl SettlementCalculator {
    /// Create new settlement calculator with default US calendar
    pub fn new() -> Self {
        Self {
            calendar: BusinessDayCalendar::us_banking(),
        }
    }

    /// Create settlement calculator with custom calendar
    pub fn with_calendar(calendar: BusinessDayCalendar) -> Self {
        Self { calendar }
    }

    /// Calculate settlement date for ACH payment
    ///
    /// ACH settlement timing:
    /// - Standard ACH: T+2 to T+4 business days
    /// - Same-day ACH: T+1 business day (if initiated before cutoff)
    /// - Next-day ACH: T+1 business day
    ///
    /// # Arguments
    /// * `initiated_at` - When the payment was initiated
    /// * `settlement_type` - Type of ACH settlement
    ///
    /// # Returns
    /// Settlement date and number of business days
    pub fn calculate_settlement_date(
        &self,
        initiated_at: DateTime<Utc>,
        settlement_type: AchSettlementType,
    ) -> SettlementDate {
        let business_days = settlement_type.business_days();

        info!(
            "Calculating settlement date: initiated={}, type={:?}, business_days={}",
            initiated_at.format("%Y-%m-%d %H:%M:%S"),
            settlement_type,
            business_days
        );

        let settlement_date = self.calendar.add_business_days_datetime(initiated_at, business_days);

        info!(
            "Settlement date calculated: {} ({} business days from {})",
            settlement_date.format("%Y-%m-%d"),
            business_days,
            initiated_at.format("%Y-%m-%d")
        );

        SettlementDate {
            settlement_date,
            initiated_at,
            business_days,
            settlement_type,
        }
    }

    /// Check if payment should be settled by now
    pub fn is_settlement_overdue(
        &self,
        initiated_at: DateTime<Utc>,
        settlement_type: AchSettlementType,
    ) -> bool {
        let expected_settlement = self.calculate_settlement_date(initiated_at, settlement_type);
        let now = Utc::now();
        
        let is_overdue = now > expected_settlement.settlement_date;
        
        if is_overdue {
            warn!(
                "Settlement overdue: expected={}, now={}",
                expected_settlement.settlement_date.format("%Y-%m-%d"),
                now.format("%Y-%m-%d")
            );
        }
        
        is_overdue
    }

    /// Get settlement status based on current time
    pub fn get_settlement_status(
        &self,
        initiated_at: DateTime<Utc>,
        settlement_type: AchSettlementType,
    ) -> SettlementStatus {
        let expected_settlement = self.calculate_settlement_date(initiated_at, settlement_type);
        let now = Utc::now();

        if now < expected_settlement.settlement_date {
            SettlementStatus::InTransit
        } else if now.date_naive() == expected_settlement.settlement_date.date_naive() {
            SettlementStatus::SettlingToday
        } else {
            SettlementStatus::Overdue
        }
    }

    /// Calculate days until settlement
    pub fn days_until_settlement(
        &self,
        initiated_at: DateTime<Utc>,
        settlement_type: AchSettlementType,
    ) -> i64 {
        let expected_settlement = self.calculate_settlement_date(initiated_at, settlement_type);
        let now = Utc::now();
        
        (expected_settlement.settlement_date.date_naive() - now.date_naive()).num_days()
    }
}

impl Default for SettlementCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// ACH settlement types with different timing
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AchSettlementType {
    /// Standard ACH (T+2 to T+4 business days) - most common
    Standard,
    
    /// Same-day ACH (T+1 business day) - requires cutoff time
    SameDay,
    
    /// Next-day ACH (T+1 business day)
    NextDay,
}

impl AchSettlementType {
    /// Get business days for settlement type
    pub fn business_days(&self) -> i64 {
        match self {
            AchSettlementType::Standard => 3, // T+3 is typical for standard ACH
            AchSettlementType::SameDay => 1,  // T+1 for same-day
            AchSettlementType::NextDay => 1,  // T+1 for next-day
        }
    }
}

/// Settlement date calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementDate {
    /// Calculated settlement date
    pub settlement_date: DateTime<Utc>,

    /// When payment was initiated
    pub initiated_at: DateTime<Utc>,

    /// Number of business days for settlement
    pub business_days: i64,

    /// Settlement type used
    pub settlement_type: AchSettlementType,
}

/// Settlement status for tracking payment lifecycle
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SettlementStatus {
    /// Payment is in transit, not yet settled
    InTransit,

    /// Payment is settling today
    SettlingToday,

    /// Payment settlement is overdue
    Overdue,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Datelike, Weekday, Duration};

    #[test]
    fn test_standard_ach_settlement() {
        let calculator = SettlementCalculator::new();

        // Monday initiation -> Thursday settlement (T+3)
        let initiated = Utc.with_ymd_and_hms(2025, 1, 6, 10, 0, 0).unwrap(); // Monday
        let result = calculator.calculate_settlement_date(initiated, AchSettlementType::Standard);

        assert_eq!(result.business_days, 3);
        // Settlement should be at least 3 business days later
        assert!(result.settlement_date > initiated);
    }

    #[test]
    fn test_same_day_ach_settlement() {
        let calculator = SettlementCalculator::new();

        // Monday initiation -> Tuesday settlement (T+1)
        let initiated = Utc.with_ymd_and_hms(2025, 1, 6, 10, 0, 0).unwrap(); // Monday
        let result = calculator.calculate_settlement_date(initiated, AchSettlementType::SameDay);

        assert_eq!(result.business_days, 1);
        // Settlement should be at least 1 business day later
        assert!(result.settlement_date > initiated);
    }

    #[test]
    fn test_settlement_overdue() {
        let calculator = SettlementCalculator::new();

        // Payment initiated 10 days ago
        let initiated = Utc::now() - Duration::days(10);

        // Standard ACH (T+3) should be overdue
        assert!(calculator.is_settlement_overdue(initiated, AchSettlementType::Standard));
    }

    #[test]
    fn test_settlement_not_overdue() {
        let calculator = SettlementCalculator::new();

        // Payment initiated today
        let initiated = Utc::now();

        // Standard ACH (T+3) should not be overdue
        assert!(!calculator.is_settlement_overdue(initiated, AchSettlementType::Standard));
    }

    #[test]
    fn test_settlement_status_in_transit() {
        let calculator = SettlementCalculator::new();

        // Payment initiated today
        let initiated = Utc::now();

        let status = calculator.get_settlement_status(initiated, AchSettlementType::Standard);
        assert_eq!(status, SettlementStatus::InTransit);
    }

    #[test]
    fn test_days_until_settlement() {
        let calculator = SettlementCalculator::new();

        // Payment initiated today
        let initiated = Utc::now();

        let days = calculator.days_until_settlement(initiated, AchSettlementType::Standard);

        // Should be at least 3 business days (could be more if weekends/holidays)
        assert!(days >= 3);
    }
}

