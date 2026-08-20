// TAG: surface=security owner=security-team rule=SEC-001
//! Idempotency key enforcement middleware for `FreshCredit` API endpoints
//!
//! Ensures that duplicate requests with the same idempotency key return the same response,
//! preventing duplicate operations (e.g., double payments, duplicate reports).

pub mod config;
pub mod middleware;
pub mod store;

pub use config::IdempotencyConfig;
pub use middleware::IdempotencyLayer;
pub use store::{IdempotencyError, IdempotencyStore, StoredResponse};
