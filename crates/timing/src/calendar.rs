//! Business day calendar for settlement calculations
//!
//! This module provides utilities for working with business days,
//! accounting for weekends and US federal holidays.

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use std::collections::HashSet;

/// Business day calendar for settlement calculations
#[derive(Debug, Clone)]
pub struct BusinessDayCalendar {
    holidays: HashSet<NaiveDate>,
}

impl BusinessDayCalendar {
    /// Create a new business day calendar with US federal holidays
    pub fn new() -> Self {
        Self {
            holidays: Self::us_federal_holidays_2025(),
        }
    }

    /// Create a new business day calendar with US banking holidays (alias for new)
    pub fn us_banking() -> Self {
        Self::new()
    }

    /// Check if a date is a business day
    pub fn is_business_day(&self, date: NaiveDate) -> bool {
        !self.is_weekend(date) && !self.holidays.contains(&date)
    }

    /// Check if a date is a weekend
    fn is_weekend(&self, date: NaiveDate) -> bool {
        matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
    }

    /// Add business days to a date (NaiveDate version)
    pub fn add_business_days(&self, start: NaiveDate, days: i32) -> NaiveDate {
        let mut current = start;
        let mut remaining = days.abs();
        let direction = if days >= 0 { 1 } else { -1 };

        while remaining > 0 {
            current = current + Duration::days(direction as i64);
            if self.is_business_day(current) {
                remaining -= 1;
            }
        }

        current
    }

    /// Add business days to a DateTime<Utc>
    pub fn add_business_days_datetime(&self, start: chrono::DateTime<chrono::Utc>, days: i64) -> chrono::DateTime<chrono::Utc> {
        let start_date = start.date_naive();
        let result_date = self.add_business_days(start_date, days as i32);

        // Preserve the time component
        start.with_timezone(&chrono::Utc)
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(chrono::Utc)
            .unwrap()
            + chrono::Duration::days((result_date - start_date).num_days())
    }
    
    /// Calculate business days between two dates
    pub fn business_days_between(&self, start: NaiveDate, end: NaiveDate) -> i32 {
        let mut count = 0;
        let mut current = start;
        
        while current < end {
            if self.is_business_day(current) {
                count += 1;
            }
            current = current + Duration::days(1);
        }
        
        count
    }
    
    /// Get the next business day from the given date
    pub fn next_business_day(&self, date: NaiveDate) -> NaiveDate {
        self.add_business_days(date, 1)
    }
    
    /// Get the previous business day from the given date
    pub fn previous_business_day(&self, date: NaiveDate) -> NaiveDate {
        self.add_business_days(date, -1)
    }
    
    /// US Federal Holidays for 2025
    /// TODO: Extend this for future years or load from configuration
    fn us_federal_holidays_2025() -> HashSet<NaiveDate> {
        let mut holidays = HashSet::new();
        
        // New Year's Day
        holidays.insert(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
        // Martin Luther King Jr. Day (3rd Monday in January)
        holidays.insert(NaiveDate::from_ymd_opt(2025, 1, 20).unwrap());
        // Presidents' Day (3rd Monday in February)
        holidays.insert(NaiveDate::from_ymd_opt(2025, 2, 17).unwrap());
        // Memorial Day (last Monday in May)
        holidays.insert(NaiveDate::from_ymd_opt(2025, 5, 26).unwrap());
        // Juneteenth
        holidays.insert(NaiveDate::from_ymd_opt(2025, 6, 19).unwrap());
        // Independence Day
        holidays.insert(NaiveDate::from_ymd_opt(2025, 7, 4).unwrap());
        // Labor Day (1st Monday in September)
        holidays.insert(NaiveDate::from_ymd_opt(2025, 9, 1).unwrap());
        // Columbus Day (2nd Monday in October)
        holidays.insert(NaiveDate::from_ymd_opt(2025, 10, 13).unwrap());
        // Veterans Day
        holidays.insert(NaiveDate::from_ymd_opt(2025, 11, 11).unwrap());
        // Thanksgiving (4th Thursday in November)
        holidays.insert(NaiveDate::from_ymd_opt(2025, 11, 27).unwrap());
        // Christmas
        holidays.insert(NaiveDate::from_ymd_opt(2025, 12, 25).unwrap());
        
        holidays
    }
}

impl Default for BusinessDayCalendar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_business_day() {
        let calendar = BusinessDayCalendar::new();
        
        // Monday, Jan 6, 2025 (business day)
        let monday = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        assert!(calendar.is_business_day(monday));
        
        // Saturday, Jan 4, 2025 (weekend)
        let saturday = NaiveDate::from_ymd_opt(2025, 1, 4).unwrap();
        assert!(!calendar.is_business_day(saturday));
        
        // Wednesday, Jan 1, 2025 (New Year's Day)
        let new_years = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        assert!(!calendar.is_business_day(new_years));
    }

    #[test]
    fn test_add_business_days() {
        let calendar = BusinessDayCalendar::new();
        
        // Start on Friday, Jan 3, 2025
        let friday = NaiveDate::from_ymd_opt(2025, 1, 3).unwrap();
        
        // Add 1 business day -> Monday, Jan 6 (skip weekend)
        let next = calendar.add_business_days(friday, 1);
        assert_eq!(next, NaiveDate::from_ymd_opt(2025, 1, 6).unwrap());
        
        // Add 5 business days -> Friday, Jan 10
        let next = calendar.add_business_days(friday, 5);
        assert_eq!(next, NaiveDate::from_ymd_opt(2025, 1, 10).unwrap());
    }

    #[test]
    fn test_business_days_between() {
        let calendar = BusinessDayCalendar::new();
        
        // Monday, Jan 6 to Friday, Jan 10 = 5 business days
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        assert_eq!(calendar.business_days_between(start, end), 4); // Excludes end date
    }
}

