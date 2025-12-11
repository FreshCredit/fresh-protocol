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
    /// Create a new business day calendar with US federal holidays for current and next year
    pub fn new() -> Self {
        let mut holidays = HashSet::new();

        // Load holidays for 2025 and 2026 to handle year transitions
        holidays.extend(Self::us_federal_holidays_for_year(2025));
        holidays.extend(Self::us_federal_holidays_for_year(2026));

        Self { holidays }
    }

    /// Create a new business day calendar with US banking holidays (alias for new)
    pub fn us_banking() -> Self {
        Self::new()
    }

    /// Create a business day calendar with custom holidays
    pub fn with_holidays(holidays: HashSet<NaiveDate>) -> Self {
        Self { holidays }
    }

    /// Add additional holidays to the calendar
    pub fn add_holidays(&mut self, additional_holidays: &[NaiveDate]) {
        for holiday in additional_holidays {
            self.holidays.insert(*holiday);
        }
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
            current += Duration::days(direction as i64);
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
            current += Duration::days(1);
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
    
    /// Calculate US Federal Holidays for a given year
    ///
    /// Dynamically calculates holidays based on federal holiday rules:
    /// - Fixed date holidays (New Year's, Juneteenth, Independence Day, Veterans Day, Christmas)
    /// - Floating holidays (MLK Day, Presidents' Day, Memorial Day, Labor Day, Columbus Day, Thanksgiving)
    fn us_federal_holidays_for_year(year: i32) -> HashSet<NaiveDate> {
        let mut holidays = HashSet::new();

        // Fixed date holidays
        // New Year's Day - January 1
        if let Some(date) = NaiveDate::from_ymd_opt(year, 1, 1) {
            holidays.insert(date);
        }
        // Juneteenth - June 19
        if let Some(date) = NaiveDate::from_ymd_opt(year, 6, 19) {
            holidays.insert(date);
        }
        // Independence Day - July 4
        if let Some(date) = NaiveDate::from_ymd_opt(year, 7, 4) {
            holidays.insert(date);
        }
        // Veterans Day - November 11
        if let Some(date) = NaiveDate::from_ymd_opt(year, 11, 11) {
            holidays.insert(date);
        }
        // Christmas - December 25
        if let Some(date) = NaiveDate::from_ymd_opt(year, 12, 25) {
            holidays.insert(date);
        }

        // Floating holidays (calculated based on weekday rules)
        // Martin Luther King Jr. Day - 3rd Monday in January
        if let Some(date) = Self::nth_weekday_of_month(year, 1, Weekday::Mon, 3) {
            holidays.insert(date);
        }
        // Presidents' Day - 3rd Monday in February
        if let Some(date) = Self::nth_weekday_of_month(year, 2, Weekday::Mon, 3) {
            holidays.insert(date);
        }
        // Memorial Day - Last Monday in May
        if let Some(date) = Self::last_weekday_of_month(year, 5, Weekday::Mon) {
            holidays.insert(date);
        }
        // Labor Day - 1st Monday in September
        if let Some(date) = Self::nth_weekday_of_month(year, 9, Weekday::Mon, 1) {
            holidays.insert(date);
        }
        // Columbus Day - 2nd Monday in October
        if let Some(date) = Self::nth_weekday_of_month(year, 10, Weekday::Mon, 2) {
            holidays.insert(date);
        }
        // Thanksgiving - 4th Thursday in November
        if let Some(date) = Self::nth_weekday_of_month(year, 11, Weekday::Thu, 4) {
            holidays.insert(date);
        }

        holidays
    }

    /// Find the nth occurrence of a weekday in a given month
    fn nth_weekday_of_month(year: i32, month: u32, weekday: Weekday, n: u32) -> Option<NaiveDate> {
        let first_of_month = NaiveDate::from_ymd_opt(year, month, 1)?;
        let first_weekday = first_of_month.weekday();

        // Calculate days until the target weekday
        let days_until = (weekday.num_days_from_monday() as i32 - first_weekday.num_days_from_monday() as i32 + 7) % 7;
        let day = 1 + days_until as u32 + (n - 1) * 7;

        NaiveDate::from_ymd_opt(year, month, day)
    }

    /// Find the last occurrence of a weekday in a given month
    fn last_weekday_of_month(year: i32, month: u32, weekday: Weekday) -> Option<NaiveDate> {
        // Start from the last day of the month and work backwards
        let next_month = if month == 12 { 1 } else { month + 1 };
        let next_year = if month == 12 { year + 1 } else { year };
        let first_of_next = NaiveDate::from_ymd_opt(next_year, next_month, 1)?;
        let last_of_month = first_of_next - Duration::days(1);

        let mut current = last_of_month;
        while current.weekday() != weekday {
            current -= Duration::days(1);
        }

        Some(current)
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

