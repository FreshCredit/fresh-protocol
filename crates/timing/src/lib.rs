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

pub mod calendar;
pub mod clock;
pub mod freshness;
pub mod retry;
pub mod settlement;
pub mod timeout;
pub mod ttl;

// Re-export commonly used types
pub use calendar::BusinessDayCalendar;
pub use clock::{Clock, MockClock, SystemClock};
pub use freshness::{DataType, FreshnessStatus, FreshnessThresholds, FreshnessValidator};
pub use retry::{ExponentialBackoff, RetryExecutor, RetryStrategy};
pub use settlement::{AchSettlementType, SettlementCalculator, SettlementDate, SettlementStatus};
pub use timeout::{TimeoutEnforcer, TimeoutError};
pub use ttl::{TtlConfig, TtlEnforcement, TtlError};
