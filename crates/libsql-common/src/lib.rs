//! Shared utilities for libSQL/Turso database connections.
//!
//! This crate provides a dual-path architecture for libSQL connections:
//! - **Direct Remote** (primary): HTTP connection to Turso Cloud
//! - **Embedded Replica** (fallback): Local cache with background sync
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
//! - [`connection`](crate::connection): Core trait definitions and configuration
//! - [`connections`](crate::connections): Concrete connection implementations
//! - [`factory`](crate::factory): Connection factory for creating connections
//! - [`connection_factory`](crate::connection_factory): Retry logic and utilities
//! - [`url_builder`](crate::url_builder): Turso URL construction utilities

// Core modules
pub mod connection;
pub mod connections;
pub mod factory;

// Existing modules
mod connection_factory;
mod url_builder;

// Re-exports for convenience
pub use connection::{
    ConnectionConfig, ConnectionHealth, ConnectionMode, DatabaseConnection,
    DatabaseConnectionExt, ReadConsistency,
};
pub use connections::{LocalConnection, RemoteConnection, ReplicaConnection};
pub use factory::ConnectionFactory;
pub use connection_factory::{with_retry, RetryConfig};
pub use url_builder::TursoUrlBuilder;

/// Version of this crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
