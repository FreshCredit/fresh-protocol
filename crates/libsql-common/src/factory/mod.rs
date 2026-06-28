//! Connection factory for creating database connections
//!
//! This module provides a factory pattern for creating different types of
//! database connections based on configuration.
//!
// TAG: surface=database owner=platform-team rule=DB-001
//! # Feature Flags
//!
//! - `remote` (default): Direct HTTP connections to Turso Cloud
//! - `local-only` (default): Local `SQLite` connections
//! - `embedded-replica`: Local cache with background sync (opt-in)
//!
//! # Example
//!
//! ```rust,no_run
//! use freshcredit_libsql_common::{
//!     ConnectionFactory, ConnectionConfig, ConnectionMode
//! };
//!
//! async fn example() -> anyhow::Result<()> {
//!     // Default: Direct remote (no feature flags needed)
//!     let config = ConnectionConfig {
//!         mode: ConnectionMode::DirectRemote,
//!         remote_url: "libsql://my-db.turso.io".to_string(),
//!         auth_token: "token".to_string(),
//!         ..Default::default()
//!     };
//!
//!     let db = ConnectionFactory::create(&config).await?;
//!     let rows = db.query("SELECT 1", vec![]).await?;
//!
//!     Ok(())
//! }
//! ```

/// Local-only connection factory
pub mod local;
/// Remote connection factory
pub mod remote;
/// Embedded replica connection factory
pub mod replica;

use std::sync::Arc;

use crate::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerConnection};
use crate::connection::{ConnectionConfig, ConnectionMode, DatabaseConnection};

// TAG: surface=database owner=platform-team rule=DB-001
/// Factory for creating database connections
#[derive(Debug)]
pub struct ConnectionFactory;

impl ConnectionFactory {
    // TAG: surface=database owner=platform-team rule=DB-001
    /// Create a new connection based on configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Configuration validation fails
    /// - Database connection fails
    ///
    /// # Feature Flags
    ///
    /// - `DirectRemote` and `LocalOnly` are always available
    /// - `EmbeddedReplica` and `Adaptive` require the `embedded-replica` feature
    ///   (if not enabled, falls back to `DirectRemote` with a warning)
    pub async fn create(config: &ConnectionConfig) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        // Validate configuration first
        config.validate()?;

        match config.mode {
            ConnectionMode::DirectRemote => Self::create_remote(config).await,
            ConnectionMode::LocalOnly => Self::create_local(config).await,

            #[cfg(feature = "embedded-replica")]
            ConnectionMode::EmbeddedReplica => Self::create_replica(config).await,

            #[cfg(feature = "embedded-replica")]
            ConnectionMode::Adaptive => {
                // For adaptive mode, start with replica
                Self::create_replica(config).await
            }
        }
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Create with automatic fallback
    ///
    /// Attempts primary mode first, falls back to secondary on failure.
    /// This is useful for high-availability scenarios.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use freshcredit_libsql_common::{
    ///     ConnectionFactory, ConnectionConfig, ConnectionMode
    /// };
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let primary = ConnectionConfig {
    ///         mode: ConnectionMode::DirectRemote,
    ///         remote_url: "libsql://primary.turso.io".to_string(),
    ///         auth_token: "token".to_string(),
    ///         ..Default::default()
    ///     };
    ///
    ///     let fallback = ConnectionConfig {
    ///         mode: ConnectionMode::LocalOnly,
    ///         local_path: Some("/app/data/fallback.db".into()),
    ///         ..Default::default()
    ///     };
    ///
    ///     let db = ConnectionFactory::create_with_fallback(&primary, &fallback).await?;
    ///     Ok(())
    /// }
    /// ```
    /// # Errors
    // TAG: surface=database owner=platform-team rule=GENERAL-001
    ///
    /// Returns an error if the operation fails.
    pub async fn create_with_fallback(
        primary: &ConnectionConfig,
        fallback: &ConnectionConfig,
    ) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        // Try primary first
        match Self::create(primary).await {
            Ok(conn) => {
                tracing::info!("Using primary connection ({:?})", primary.mode);
                Ok(conn)
            }
            Err(e) => {
                tracing::warn!("Primary connection failed ({}), trying fallback", e);

                // Try fallback
                match Self::create(fallback).await {
                    Ok(conn) => {
                        tracing::info!("Using fallback connection ({:?})", fallback.mode);
                        Ok(conn)
                    }
                    Err(e2) => {
                        tracing::error!("Fallback connection also failed: {}", e2);
                        Err(anyhow::anyhow!(
                            "Both primary and fallback connections failed: primary={e}, fallback={e2}"
                        ))
                    }
                }
            }
        }
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Create from environment variables
    ///
    /// Uses the following environment variables:
    /// - `LIBSQL_MODE`: Connection mode (remote, replica, local, adaptive)
    /// - `TURSO_URL` or `LIBSQL_URL`: Remote database URL
    /// - `TURSO_AUTH_TOKEN` or `LIBSQL_AUTH_TOKEN`: Authentication token
    /// - `LIBSQL_LOCAL_PATH`: Local database path
    /// - `LIBSQL_ENABLE_FALLBACK`: Enable automatic fallback
    /// - `LIBSQL_SYNC_INTERVAL_SECS`: Sync interval for replica mode
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use freshcredit_libsql_common::ConnectionFactory;
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let conn = ConnectionFactory::from_env().await?;
    ///     let rows = conn.query("SELECT 1", vec![]).await?;
    ///     Ok(())
    /// }
    /// ```
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn from_env() -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let config = ConnectionConfig::from_env()?;
        Self::create(&config).await
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Create a connection with circuit breaker protection
    ///
    /// Wraps the underlying connection with a circuit breaker that will
    /// fail fast when the database is unavailable, preventing cascading failures.
    ///
    /// # Circuit Breaker Configuration
    ///
    /// Environment variables:
    /// - `DB_CB_FAILURE_THRESHOLD`: Failures before opening (default: 5)
    /// - `DB_CB_RECOVERY_TIMEOUT_SECS`: Seconds before retry (default: 30)
    /// - `DB_CB_HALF_OPEN_MAX_CALLS`: Test calls in half-open (default: 3)
    /// - `DB_CB_SUCCESS_THRESHOLD`: Successes to close (default: 2)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use freshcredit_libsql_common::{ConnectionFactory, DatabaseConnection};
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let conn = ConnectionFactory::create_with_circuit_breaker_from_env().await?;
    ///
    ///     // Operations automatically protected by circuit breaker
    ///     let rows = conn.query("SELECT * FROM users", vec![]).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn create_with_circuit_breaker(
        config: &ConnectionConfig,
        cb_config: CircuitBreakerConfig,
    ) -> anyhow::Result<Arc<CircuitBreakerConnection>> {
        let inner = Self::create(config).await?;
        Ok(CircuitBreakerConnection::new(inner, cb_config))
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Create a connection with circuit breaker from environment
    ///
    /// Uses both database connection env vars and circuit breaker env vars.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use freshcredit_libsql_common::{ConnectionFactory, DatabaseConnection};
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let conn = ConnectionFactory::create_with_circuit_breaker_from_env().await?;
    ///     let rows = conn.query("SELECT 1", vec![]).await?;
    ///     Ok(())
    /// }
    /// ```
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn create_with_circuit_breaker_from_env(
    ) -> anyhow::Result<Arc<CircuitBreakerConnection>> {
        let config = ConnectionConfig::from_env()?;
        let cb_config = CircuitBreakerConfig::from_env();
        Self::create_with_circuit_breaker(&config, cb_config).await
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Create with circuit breaker and automatic fallback
    ///
    /// Combines circuit breaker protection with fallback to a secondary
    /// database if the primary fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use freshcredit_libsql_common::{
    ///     ConnectionFactory, ConnectionConfig, ConnectionMode, CircuitBreakerConfig
    /// };
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let primary = ConnectionConfig {
    ///         mode: ConnectionMode::DirectRemote,
    ///         remote_url: "libsql://primary.turso.io".to_string(),
    ///         auth_token: "token".to_string(),
    ///         ..Default::default()
    ///     };
    ///
    ///     let fallback = ConnectionConfig {
    ///         mode: ConnectionMode::LocalOnly,
    ///         local_path: Some("/app/data/fallback.db".into()),
    ///         ..Default::default()
    ///     };
    ///
    ///     let cb_config = CircuitBreakerConfig::default();
    ///     let conn = ConnectionFactory::create_with_cb_and_fallback(
    ///         &primary, &fallback, cb_config
    ///     ).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn create_with_cb_and_fallback(
        // TAG: surface=database owner=platform-team rule=GENERAL-001
        primary: &ConnectionConfig,
        fallback: &ConnectionConfig,
        cb_config: CircuitBreakerConfig,
    ) -> anyhow::Result<Arc<CircuitBreakerConnection>> {
        // Try to create primary with circuit breaker
        match Self::create_with_circuit_breaker(primary, cb_config).await {
            Ok(conn) => {
                tracing::info!(
                    "Created circuit breaker protected connection with primary ({:?})",
                    primary.mode
                );
                Ok(conn)
            }
            Err(e) => {
                tracing::warn!(
                    "Primary connection failed ({}), trying fallback without CB",
                    e
                );

                // Try fallback without circuit breaker (local should be reliable)
                match Self::create(fallback).await {
                    Ok(conn) => {
                        tracing::info!("Using fallback connection ({:?})", fallback.mode);
                        // Still wrap with circuit breaker for consistency
                        Ok(CircuitBreakerConnection::new(conn, cb_config))
                    }
                    Err(e2) => {
                        tracing::error!("Fallback connection also failed: {}", e2);
                        Err(anyhow::anyhow!(
                            "Both primary and fallback connections failed: primary={e}, fallback={e2}"
                        ))
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
