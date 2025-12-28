//! Shared utilities for libSQL/Turso database connections.
//!
//! This crate provides centralized utilities for:
//! - Per-user Turso URL construction with consistent sanitization
//! - Database connection configuration
//! - Connection factory with retry logic
//!
//! # URL Construction
//!
//! Use [`TursoUrlBuilder`] for consistent per-user database URL generation:
//!
//! ```rust
//! use freshcredit_libsql_common::TursoUrlBuilder;
//!
//! let builder = TursoUrlBuilder::new("devonshigaki");
//! let url = builder.user_database_url("user@example.com");
//! // Returns: libsql://user-user-example-com-{org}.aws-us-west-2.turso.io
//! ```
//!
//! # Connection Retry
//!
//! Use [`with_retry`] for resilient database connections:
//!
//! ```ignore
//! use freshcredit_libsql_common::{RetryConfig, with_retry};
//!
//! let config = RetryConfig::default();
//! let client = with_retry(&config, || async {
//!     CloudClient::new(&url, &token).await
//! }).await?;
//! ```

mod connection_factory;
mod url_builder;

pub use connection_factory::{with_retry, RetryConfig};
pub use url_builder::TursoUrlBuilder;
