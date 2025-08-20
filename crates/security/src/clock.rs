//! Clock utilities for FreshCredit

use chrono::{DateTime, Utc};

/// Clock trait for time operations
pub trait Clock {
    /// Get current UTC time
    fn now(&self) -> DateTime<Utc>;
}

/// System clock implementation
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Mock clock for testing
pub struct MockClock {
    pub time: DateTime<Utc>,
}

impl Clock for MockClock {
    fn now(&self) -> DateTime<Utc> {
        self.time
    }
}
