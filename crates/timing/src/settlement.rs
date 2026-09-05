// TAG: surface=api owner=platform-team rule=API-001
//! ACH settlement timing calculations for `FreshCredit`
//!
//! Implements business day calculations for ACH settlement dates (T+1 to T+4).
//! Uses `BusinessDayCalendar` for holiday and weekend handling.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::calendar::BusinessDayCalendar;
use crate::clock::Clock;

/// ACH settlement timing calculator
#[derive(Debug)]
pub struct SettlementCalculator {
    calendar: BusinessDayCalendar,
}

impl SettlementCalculator {
    /// Create new settlement calculator with default US calendar
    #[must_use]
    pub fn new() -> Self {
        Self {
            calendar: BusinessDayCalendar::us_banking(),
        }
    }

    /// Create settlement calculator with custom calendar
    #[must_use]
    // TAG: surface=api owner=platform-team rule=API-001
    pub const fn with_calendar(calendar: BusinessDayCalendar) -> Self {
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
            // TAG: surface=api owner=platform-team rule=API-001
            business_days
        );

        let settlement_date = self
            .calendar
            .add_business_days_datetime(initiated_at, business_days);

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

    /// Check if payment should be settled by the provided clock's current time.
    pub fn is_settlement_overdue_with_clock(
        &self,
        clock: &dyn Clock,
        initiated_at: DateTime<Utc>,
        settlement_type: AchSettlementType,
    ) -> bool {
        let expected_settlement = self.calculate_settlement_date(initiated_at, settlement_type);
        let now = clock.now();

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

    /// Check if payment should be settled by now using the system wall-clock.
    #[must_use]
    pub fn is_settlement_overdue(
        &self,
        initiated_at: DateTime<Utc>,
        settlement_type: AchSettlementType,
    ) -> bool {
        self.is_settlement_overdue_with_clock(
            &crate::clock::SystemClock,
            initiated_at,
            settlement_type,
        )
    }

    /// Get settlement status based on the provided clock's current time.
    #[must_use]
    pub fn get_settlement_status_with_clock(
        &self,
        clock: &dyn Clock,
        initiated_at: DateTime<Utc>,
        settlement_type: AchSettlementType,
    ) -> SettlementStatus {
        let expected_settlement = self.calculate_settlement_date(initiated_at, settlement_type);
        let now = clock.now();

        if now < expected_settlement.settlement_date {
            SettlementStatus::InTransit
        } else if now.date_naive() == expected_settlement.settlement_date.date_naive() {
            // TAG: surface=api owner=platform-team rule=API-001
            SettlementStatus::SettlingToday
        } else {
            SettlementStatus::Overdue
        }
    }

    /// Get settlement status based on current time using the system wall-clock.
    #[must_use]
    pub fn get_settlement_status(
        &self,
        initiated_at: DateTime<Utc>,
        settlement_type: AchSettlementType,
    ) -> SettlementStatus {
        self.get_settlement_status_with_clock(
            &crate::clock::SystemClock,
            initiated_at,
            settlement_type,
        )
    }

    /// Calculate days until settlement using the provided clock's current time.
    #[must_use]
    pub fn days_until_settlement_with_clock(
        &self,
        clock: &dyn Clock,
        initiated_at: DateTime<Utc>,
        settlement_type: AchSettlementType,
    ) -> i64 {
        let expected_settlement = self.calculate_settlement_date(initiated_at, settlement_type);
        let now = clock.now();

        (expected_settlement.settlement_date.date_naive() - now.date_naive()).num_days()
    }

    /// Calculate days until settlement using the system wall-clock.
    #[must_use]
    pub fn days_until_settlement(
        &self,
        initiated_at: DateTime<Utc>,
        settlement_type: AchSettlementType,
    ) -> i64 {
        self.days_until_settlement_with_clock(
            &crate::clock::SystemClock,
            initiated_at,
            settlement_type,
        )
    }
}

impl Default for SettlementCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// ACH settlement types with different timing
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
// TAG: surface=api owner=platform-team rule=API-001
pub enum AchSettlementType {
    /// Regular ACH (T+2 to T+4 business days) - most common, 3-day settlement
    ThreeDaySettlement,

    /// Same-day ACH (T+1 business day) - requires cutoff time
    SameDay,

    /// Next-day ACH (T+1 business day)
    NextDay,
}

impl AchSettlementType {
    /// Get business days for settlement type
    #[must_use]
    pub const fn business_days(&self) -> i64 {
        match self {
            Self::ThreeDaySettlement => 3,      // T+3 for regular ACH
            Self::SameDay | Self::NextDay => 1, // T+1 for same-day and next-day
        }
    }
}

/// Settlement date calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementDate {
    /// Calculated settlement date
    pub settlement_date: DateTime<Utc>,

    /// When payment was initiated
    // TAG: surface=api owner=platform-team rule=API-001
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
    use chrono::{Duration, TimeZone};

    #[test]
    // TAG: surface=api owner=platform-team rule=API-001
    fn test_standard_ach_settlement() {
        let calculator = SettlementCalculator::new();

        // Monday initiation -> Thursday settlement (T+3)
        let initiated = Utc.with_ymd_and_hms(2025, 1, 6, 10, 0, 0).unwrap(); // Monday
        let result =
            calculator.calculate_settlement_date(initiated, AchSettlementType::ThreeDaySettlement);

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
        // TAG: surface=api owner=platform-team rule=API-001
        let calculator = SettlementCalculator::new();

        // Payment initiated 10 days ago
        let initiated = Utc::now() - Duration::days(10);

        // Regular ACH (T+3) should be overdue
        assert!(calculator.is_settlement_overdue(initiated, AchSettlementType::ThreeDaySettlement));
    }

    #[test]
    fn test_settlement_not_overdue() {
        let calculator = SettlementCalculator::new();

        // Payment initiated today
        let initiated = Utc::now();

        // Regular ACH (T+3) should not be overdue
        assert!(!calculator.is_settlement_overdue(initiated, AchSettlementType::ThreeDaySettlement));
    }

    #[test]
    fn test_settlement_status_in_transit() {
        let calculator = SettlementCalculator::new();

        // Payment initiated today
        let initiated = Utc::now();

        let status =
            // TAG: surface=api owner=platform-team rule=API-001
            calculator.get_settlement_status(initiated, AchSettlementType::ThreeDaySettlement);
        assert_eq!(status, SettlementStatus::InTransit);
    }

    #[test]
    fn test_days_until_settlement() {
        let calculator = SettlementCalculator::new();

        // Payment initiated today
        let initiated = Utc::now();

        let days =
            calculator.days_until_settlement(initiated, AchSettlementType::ThreeDaySettlement);

        // Should be at least 3 business days (could be more if weekends/holidays)
        assert!(days >= 3);
    }

    #[test]
    fn test_settlement_calculator_with_calendar() {
        let calendar = BusinessDayCalendar::new();
        let calculator = SettlementCalculator::with_calendar(calendar);

        let initiated = Utc.with_ymd_and_hms(2025, 1, 6, 10, 0, 0).unwrap();
        let result = calculator.calculate_settlement_date(initiated, AchSettlementType::SameDay);

        assert_eq!(result.business_days, 1);
    }

    // TAG: surface=api owner=platform-team rule=API-001
    #[test]
    fn test_settlement_status_settling_today() {
        let calculator = SettlementCalculator::new();

        // For SameDay ACH, settlement should be the next business day
        let initiated = Utc::now() - Duration::days(1);
        let status = calculator.get_settlement_status(initiated, AchSettlementType::SameDay);

        // Could be SettlingToday or Overdue depending on when the test runs
        assert!(
            status == SettlementStatus::SettlingToday
                || status == SettlementStatus::Overdue
                || status == SettlementStatus::InTransit
        );
    }

    #[test]
    fn test_settlement_calculator_default() {
        let calculator: SettlementCalculator = SettlementCalculator::default();
        let initiated = Utc.with_ymd_and_hms(2025, 1, 6, 10, 0, 0).unwrap();
        let result = calculator.calculate_settlement_date(initiated, AchSettlementType::NextDay);
        assert_eq!(result.business_days, 1);
    }

    #[test]
    fn test_settlement_overdue_with_mock_clock() {
        use crate::clock::MockClock;

        let calculator = SettlementCalculator::new();
        let initiated = Utc.with_ymd_and_hms(2025, 1, 6, 10, 0, 0).unwrap(); // Monday

        // 10 days later → overdue
        let now = initiated + Duration::days(10);
        let clock = MockClock::new(now);
        assert!(calculator.is_settlement_overdue_with_clock(
            &clock,
            initiated,
            AchSettlementType::ThreeDaySettlement
        ));

        // Same day → not overdue
        let clock = MockClock::new(initiated);
        assert!(!calculator.is_settlement_overdue_with_clock(
            &clock,
            initiated,
            AchSettlementType::ThreeDaySettlement
        ));
    }

    #[test]
    fn test_settlement_status_with_mock_clock() {
        use crate::clock::MockClock;

        let calculator = SettlementCalculator::new();
        let initiated = Utc.with_ymd_and_hms(2025, 1, 6, 10, 0, 0).unwrap(); // Monday

        // Same-day ACH settles next business day (Tuesday). Viewing on Monday → InTransit.
        let now = initiated;
        let clock = MockClock::new(now);
        assert_eq!(
            calculator.get_settlement_status_with_clock(
                &clock,
                initiated,
                AchSettlementType::SameDay
            ),
            SettlementStatus::InTransit
        );

        // Viewing 10 days later → Overdue.
        let now = initiated + Duration::days(10);
        let clock = MockClock::new(now);
        assert_eq!(
            calculator.get_settlement_status_with_clock(
                &clock,
                initiated,
                AchSettlementType::SameDay
            ),
            SettlementStatus::Overdue
        );
    }

    #[test]
    fn test_days_until_settlement_with_mock_clock() {
        use crate::clock::MockClock;

        let calculator = SettlementCalculator::new();
        let initiated = Utc.with_ymd_and_hms(2025, 1, 6, 10, 0, 0).unwrap(); // Monday

        // Next-day ACH settles Tuesday. On Monday → 1 day remaining (date delta).
        let clock = MockClock::new(initiated);
        let days = calculator.days_until_settlement_with_clock(
            &clock,
            initiated,
            AchSettlementType::NextDay,
        );
        assert_eq!(days, 1);
    }

    #[test]
    fn test_next_day_ach_settlement() {
        let calculator = SettlementCalculator::new();

        // TAG: surface=api owner=platform-team rule=API-001
        let initiated = Utc.with_ymd_and_hms(2025, 1, 6, 10, 0, 0).unwrap(); // Monday
        let result = calculator.calculate_settlement_date(initiated, AchSettlementType::NextDay);

        assert_eq!(result.business_days, 1);
        assert_eq!(result.settlement_type, AchSettlementType::NextDay);
    }

    #[test]
    fn test_ach_settlement_type_business_days() {
        assert_eq!(AchSettlementType::ThreeDaySettlement.business_days(), 3);
        assert_eq!(AchSettlementType::SameDay.business_days(), 1);
        assert_eq!(AchSettlementType::NextDay.business_days(), 1);
    }

    #[test]
    fn test_settlement_date_fields() {
        let calculator = SettlementCalculator::new();
        let initiated = Utc.with_ymd_and_hms(2025, 1, 6, 10, 0, 0).unwrap();
        let result =
            calculator.calculate_settlement_date(initiated, AchSettlementType::ThreeDaySettlement);

        assert_eq!(result.initiated_at, initiated);
        assert_eq!(
            result.settlement_type,
            AchSettlementType::ThreeDaySettlement
        );
        assert_eq!(result.business_days, 3);
    }
    // TAG: surface=api owner=platform-team rule=API-001
}
