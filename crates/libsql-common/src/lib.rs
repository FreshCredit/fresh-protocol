//! Shared utilities for libSQL/Turso database connections.
//!
//! This crate provides a dual-path architecture for libSQL connections:
//! - **Direct Remote** (default): HTTP connection to Turso Cloud
//! - **Embedded Replica** (opt-in): Local cache with background sync
//!
//! # Feature Flags
//!
//! - `remote` (default): Enable direct remote connections to Turso Cloud
//! - `local-only` (default): Enable local-only `SQLite` connections (for testing)
//! - `embedded-replica`: Enable embedded replica with background sync (**opt-in**)
//! - `dual-path`: Convenience flag to enable all connection modes
//!
//! ## Default Behavior (No Feature Flags Needed)
//!
//! ```toml
//! [dependencies]
//! freshcredit-libsql-common = { path = "../../db/libsql/common" }
//! ```
//!
//! This gives you `DirectRemote` and `LocalOnly` modes.
//!
//! ## Opt-In to Embedded Replica
//!
//! ```toml
//! [dependencies]
//! freshcredit-libsql-common = {
//!     path = "../../db/libsql/common",
//!     features = ["embedded-replica"]
//! }
//! ```
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Application Layer                         │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │              DatabaseConnection (trait)                │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                 ConnectionFactory                            │
//! └─────────────────────────────────────────────────────────────┘
//!           │                              │
//!           ▼                              ▼
//! ┌─────────────────────┐    ┌─────────────────────────┐
//! │   Direct Remote     │    │   Embedded Replica      │
//! │ Builder::new_remote │    │ Builder::new_remote_    │
//! │                     │    │        replica()        │
//! └─────────────────────┘    └─────────────────────────┘
//! ```
//!
//! # Quick Start
//!
//! ## Using the Connection Factory
//!
//! ```rust,no_run
//! use freshcredit_libsql_common::{
//!     ConnectionFactory, ConnectionConfig, ConnectionMode
//! };
//!
//! async fn example() -> anyhow::Result<()> {
//!     // Create configuration
//!     let config = ConnectionConfig {
//!         mode: ConnectionMode::EmbeddedReplica,
//!         remote_url: "libsql://my-db.turso.io".to_string(),
//!         auth_token: "my-token".to_string(),
//!         local_path: Some("/app/data/local.db".into()),
//!         sync_interval_secs: Some(60),
//!         ..Default::default()
//!     };
//!
//!     // Create connection
//!     let db = ConnectionFactory::create(&config).await?;
//!
//!     // Use the connection
//!     let rows = db.query("SELECT * FROM users", vec![]).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Using Environment Variables
//!
//! ```rust,no_run
//! use freshcredit_libsql_common::ConnectionFactory;
//!
//! async fn example() -> anyhow::Result<()> {
//!     // Set environment variables:
//!     // LIBSQL_MODE=replica
//!     // TURSO_URL=libsql://my-db.turso.io
//!     // TURSO_AUTH_TOKEN=my-token
//!     // LIBSQL_LOCAL_PATH=/app/data/local.db
//!
//!     let db = ConnectionFactory::from_env().await?;
//!     let rows = db.query("SELECT 1", vec![]).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## With Fallback
//!
//! ```rust,no_run
//! use freshcredit_libsql_common::{
//!     ConnectionFactory, ConnectionConfig, ConnectionMode
//! };
//!
//! async fn example() -> anyhow::Result<()> {
//!     let primary = ConnectionConfig {
//!         mode: ConnectionMode::DirectRemote,
//!         remote_url: "libsql://primary.turso.io".to_string(),
//!         auth_token: "token".to_string(),
//!         ..Default::default()
//!     };
//!
//!     let fallback = ConnectionConfig {
//!         mode: ConnectionMode::LocalOnly,
//!         local_path: Some("/app/data/fallback.db".into()),
//!         ..Default::default()
//!     };
//!
//!     // Will try primary first, then fallback
//!     let db = ConnectionFactory::create_with_fallback(&primary, &fallback).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Feature Flags
//!
//! The underlying `libsql` crate supports feature flags for different connection modes:
//! - `remote`: Enable direct remote connections
//! - `replication`: Enable embedded replica connections
//!
//! These are enabled by default in the workspace configuration.
//!
//! # Modules
//!
//! - `connection`: Core trait definitions and configuration
//! - `connections`: Concrete connection implementations
//! - `factory`: Connection factory for creating connections
//! - `connection_factory`: Retry logic and utilities
//! - `url_builder`: Turso URL construction utilities

// Core modules
#![allow(clippy::wildcard_imports)]
#![allow(missing_docs)]

pub mod circuit_breaker;
pub mod connection;
pub mod connections;
pub mod factory;
pub mod security;

// Existing modules
mod connection_factory;
mod url_builder;

// Re-exports for convenience
pub use circuit_breaker::{
    CircuitBreakerConfig,
    CircuitBreakerConnection,
    CircuitBreakerError,
    CircuitBreakerState,
    CircuitBreakerStats,
};
pub use connection::{
    ConnectionConfig,
    ConnectionHealth,
    ConnectionMode,
    DatabaseConnection,
    DatabaseConnectionExt,
};

#[cfg(feature = "embedded-replica")]
pub use connection::ReadConsistency;
pub use connections::{
    LocalConnection,
    RemoteConnection,
};

pub use connection_factory::{
    with_retry,
    RetryConfig,
};
#[cfg(feature = "embedded-replica")]
pub use connections::ReplicaConnection;
pub use factory::ConnectionFactory;
pub use url_builder::TursoUrlBuilder;

/// Version of this crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
