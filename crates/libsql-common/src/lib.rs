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

mod url_builder;

pub use url_builder::TursoUrlBuilder;

