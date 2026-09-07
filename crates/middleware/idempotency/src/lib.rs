// TAG: surface=security owner=security-team rule=SEC-001
//! Idempotency key enforcement middleware for `FreshCredit` API endpoints
//!
//! Ensures that duplicate requests with the same idempotency key return the same response,
//! preventing duplicate operations (e.g., double payments, duplicate reports).

#![forbid(unsafe_code)]

pub mod config;
pub mod durable;
pub mod middleware;
pub mod store;

pub use config::IdempotencyConfig;
pub use durable::{ClaimOutcome, DurableIdempotencyError, DurableIdempotencyStore};
pub use middleware::IdempotencyLayer;
pub use store::{IdempotencyError, IdempotencyStore, LookupOutcome, ResponseStore, StoredResponse};
