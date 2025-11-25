//! FreshCredit Core Timing Module
//!
//! This module provides timing abstractions and utilities for the FreshCredit platform.
//! All timing logic should use these abstractions rather than direct time operations.
//!
//! ## Modules
//!
//! - `clock`: Clock abstraction for testable time operations
//! - `ttl`: TTL (Time-To-Live) enforcement for ephemeral data
//! - `freshness`: Data freshness validation
//! - `retry`: Retry strategies with exponential backoff
//! - `timeout`: Timeout enforcement for async operations
//! - `calendar`: Business day calendar for settlement calculations
//! - `settlement`: ACH settlement timing calculations

pub mod clock;
pub mod ttl;
pub mod freshness;
pub mod retry;
pub mod timeout;
pub mod calendar;
pub mod settlement;

// Re-export commonly used types
pub use clock::{Clock, SystemClock, MockClock};
pub use ttl::{TtlEnforcement, TtlConfig, TtlError};
pub use freshness::{FreshnessValidator, FreshnessStatus, FreshnessThresholds, DataType};
pub use retry::{RetryStrategy, ExponentialBackoff, RetryExecutor};
pub use timeout::{TimeoutEnforcer, TimeoutError};
pub use calendar::BusinessDayCalendar;
pub use settlement::{SettlementCalculator, SettlementDate, SettlementStatus, AchSettlementType};

