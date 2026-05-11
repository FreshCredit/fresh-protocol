//! Unified database connection interface for libSQL
//!
//! This module provides a trait-based abstraction over different libSQL connection modes:
//! - Direct remote (HTTP to Turso)
//! - Embedded replica (local + background sync)
//! - Local only (file-based `SQLite`)
//!
//! # Example
//!
//! ```rust,no_run
//! use freshcredit_libsql_common::{ConnectionFactory, ConnectionConfig, ConnectionMode};
//!
//! async fn example() -> anyhow::Result<()> {
//!     let config = ConnectionConfig {
//!         mode: ConnectionMode::EmbeddedReplica,
//!         remote_url: "libsql://my-db.turso.io".to_string(),
//!         auth_token: "token".to_string(),
//!         local_path: Some("/app/data/local.db".into()),
//!         ..Default::default()
//!     };
//!
//!     let db = ConnectionFactory::create(&config).await?;
//!     let rows = db.query("SELECT 1", vec![]).await?;
//!     Ok(())
//! }
//! ```

pub mod types;

mod helpers;
mod impls;

#[cfg(test)]
mod tests;

pub use types::*;
