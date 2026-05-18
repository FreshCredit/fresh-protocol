//! Clock abstraction for time operations
//!
//! This module provides a trait-based clock abstraction that allows for testable
//! time operations throughout the `FreshCredit` codebase.

use chrono::{DateTime, Utc};

/// Clock abstraction for time operations
///
/// This trait should be used instead of calling `Utc::now()` directly,
/// as it allows for deterministic testing with `MockClock`.
pub trait Clock: Send + Sync {
    /// Get current UTC time
    fn now(&self) -> DateTime<Utc>;

    /// Get current Unix timestamp in seconds
    fn timestamp(&self) -> i64 {
        self.now().timestamp()
    }

    /// Get current Unix timestamp in milliseconds
    fn timestamp_millis(&self) -> i64 {
        self.now().timestamp_millis()
    }
}

/// System clock using actual time
///
/// This is the production implementation that returns the actual current time.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Mock clock for testing
///
/// This implementation allows tests to control the current time,
/// enabling deterministic testing of time-dependent logic.
#[derive(Debug, Clone)]
pub struct MockClock {
    current_time: DateTime<Utc>,
}

impl MockClock {
    /// Create a new mock clock with the given time
    #[must_use]
    pub const fn new(time: DateTime<Utc>) -> Self {
        Self { current_time: time }
    }

    /// Advance the clock by the given duration
    pub fn advance(&mut self, duration: chrono::Duration) {
        self.current_time += duration;
    }

    /// Set the clock to a specific time
    pub fn set(&mut self, time: DateTime<Utc>) {
        self.current_time = time;
    }
}

impl Clock for MockClock {
    fn now(&self) -> DateTime<Utc> {
        self.current_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_system_clock() {
        let clock = SystemClock;
        let now1 = clock.now();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let now2 = clock.now();
        assert!(now2 > now1);
    }

    #[test]
    fn test_mock_clock() {
        let time = Utc::now();
        let mut clock = MockClock::new(time);

        assert_eq!(clock.now(), time);

        clock.advance(Duration::hours(1));
        assert_eq!(clock.now(), time + Duration::hours(1));

        let new_time = time + Duration::days(1);
        clock.set(new_time);
        assert_eq!(clock.now(), new_time);
    }

    #[test]
    fn test_timestamp_methods() {
        let time = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let clock = MockClock::new(time);

        assert_eq!(clock.timestamp(), 1735689600);
        assert_eq!(clock.timestamp_millis(), 1735689600000);
    }
}
