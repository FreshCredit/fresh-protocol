// TAG: surface=api owner=platform-team rule=API-001
//! Clock abstraction for time operations
//!
//! This module provides a trait-based clock abstraction that allows for testable
//! time operations throughout the `FreshCredit` codebase.
//!
//! # Wall-clock vs monotonic time
//!
//! * `now()` returns wall-clock UTC (`DateTime<Utc>`). Use it for user-facing
//!   timestamps, audit timestamps, and TTL/freshness decisions when the host
//!   clock is trusted.
//! * `monotonic_now()` returns an `std::time::Instant`. Use it for duration
//!   measurements, timeouts, and rate-limit refills so OS clock jumps do not
//!   skew the result.
//! * `uncertainty()` is reserved for future TrueTime-style bounded uncertainty.
//!   It currently returns `None` because the system relies on the host NTP
//!   source and has no independent time reference.
//!
//! For distributed correctness (LWW conflict resolution, settlement, blockchain
//! anchoring), prefer server/cloud timestamps and document a maximum tolerated
//! skew rather than relying on client wall clocks.

use chrono::{DateTime, Utc};
use std::time::{Duration, Instant};

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

    /// Get a monotonic `Instant` suitable for measuring elapsed durations.
    ///
    /// Default implementation returns `Instant::now()`; production code should
    /// use this (or `std::time::Instant` directly) whenever measuring durations,
    /// rather than subtracting two `Utc::now()` values.
    fn monotonic_now(&self) -> Instant {
        Instant::now()
    }

    /// Estimated uncertainty bounds for the wall-clock time, if known.
    ///
    /// A `Some` value means the real time is within ±duration of `now()`.
    /// This is a placeholder for future NTP/TrueTime integration.
    fn uncertainty(&self) -> Option<Duration> {
        None
    }
}

/// System clock using actual time
///
/// This is the production implementation that returns the actual current time.
// TAG: surface=api owner=platform-team rule=API-001
#[derive(Debug)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn monotonic_now(&self) -> Instant {
        Instant::now()
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
    // TAG: surface=api owner=platform-team rule=API-001
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
    #[cfg(not(miri))]
    fn test_system_clock() {
        let clock = SystemClock;
        let now1 = clock.now();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let now2 = clock.now();
        // TAG: surface=api owner=platform-team rule=API-001
        assert!(now2 > now1);
    }

    #[test]
    #[cfg(not(miri))]
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

        assert_eq!(clock.timestamp(), 1_735_689_600);
        assert_eq!(clock.timestamp_millis(), 1_735_689_600_000);
    }
    // TAG: surface=api owner=platform-team rule=API-001
}
