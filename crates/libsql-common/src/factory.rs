//! Connection factory for creating database connections
//!
//! This module provides a factory pattern for creating different types of
//! database connections based on configuration.
//!
//! # Feature Flags
//!
//! - `remote` (default): Direct HTTP connections to Turso Cloud
//! - `local-only` (default): Local SQLite connections
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

use std::sync::Arc;

use crate::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerConnection};
use crate::connection::{ConnectionConfig, ConnectionMode, DatabaseConnection};
use crate::connections::{LocalConnection, RemoteConnection};

#[cfg(feature = "embedded-replica")]
use crate::connections::ReplicaConnection;

/// Factory for creating database connections
pub struct ConnectionFactory;

impl ConnectionFactory {
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

    /// Create a remote connection
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use freshcredit_libsql_common::ConnectionFactory;
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let conn = ConnectionFactory::create_remote_with_url(
    ///         "libsql://my-db.turso.io",
    ///         "my-token"
    ///     ).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn create_remote(
        config: &ConnectionConfig,
    ) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let conn = RemoteConnection::connect(&config.remote_url, &config.auth_token).await?;
        Ok(Arc::new(conn))
    }

    /// Create a remote connection with explicit URL and token
    pub async fn create_remote_with_url(
        url: &str,
        token: &str,
    ) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let conn = RemoteConnection::connect(url, token).await?;
        Ok(Arc::new(conn))
    }

    /// Create an embedded replica connection
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use freshcredit_libsql_common::ConnectionFactory;
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let conn = ConnectionFactory::create_replica_with_params(
    ///         "/app/data/local.db",
    ///         "libsql://my-db.turso.io",
    ///         "my-token",
    ///         Some(60)
    ///     ).await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Feature Flag
    ///
    /// This method requires the `embedded-replica` feature to be enabled.
    #[cfg(feature = "embedded-replica")]
    pub async fn create_replica(
        config: &ConnectionConfig,
    ) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let local_path = config
            .local_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Local path required for replica mode"))?;

        let conn = ReplicaConnection::connect(
            local_path,
            &config.remote_url,
            &config.auth_token,
            config.sync_interval_secs,
        )
        .await?;

        Ok(Arc::new(conn))
    }

    /// Create an embedded replica connection with explicit parameters
    ///
    /// # Feature Flag
    ///
    /// This method requires the `embedded-replica` feature to be enabled.
    #[cfg(feature = "embedded-replica")]
    pub async fn create_replica_with_params(
        local_path: impl AsRef<std::path::Path>,
        remote_url: &str,
        auth_token: &str,
        sync_interval_secs: Option<u64>,
    ) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let conn =
            ReplicaConnection::connect(local_path, remote_url, auth_token, sync_interval_secs)
                .await?;

        Ok(Arc::new(conn))
    }

    /// Create a local-only connection
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use freshcredit_libsql_common::ConnectionFactory;
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let conn = ConnectionFactory::create_local_with_path("/app/data/local.db").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn create_local(
        config: &ConnectionConfig,
    ) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let local_path = config
            .local_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Local path required for local mode"))?;

        let conn = LocalConnection::connect(local_path).await?;
        Ok(Arc::new(conn))
    }

    /// Create a local-only connection with explicit path
    pub async fn create_local_with_path(
        path: impl AsRef<std::path::Path>,
    ) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let conn = LocalConnection::connect(path).await?;
        Ok(Arc::new(conn))
    }

    /// Create an in-memory connection (useful for testing)
    ///
    /// # Example
    ///
    /// ```rust
    /// use freshcredit_libsql_common::ConnectionFactory;
    ///
    /// async fn example() -> anyhow::Result<()> {
    ///     let conn = ConnectionFactory::create_in_memory().await?;
    ///     let rows = conn.query("SELECT 1", vec![]).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn create_in_memory() -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let conn = LocalConnection::in_memory().await?;
        Ok(Arc::new(conn))
    }

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
    pub async fn from_env() -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let config = ConnectionConfig::from_env()?;
        Self::create(&config).await
    }

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
    pub async fn create_with_circuit_breaker(
        config: &ConnectionConfig,
        cb_config: CircuitBreakerConfig,
    ) -> anyhow::Result<Arc<CircuitBreakerConnection>> {
        let inner = Self::create(config).await?;
        Ok(CircuitBreakerConnection::new(inner, cb_config))
    }

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
    pub async fn create_with_circuit_breaker_from_env(
    ) -> anyhow::Result<Arc<CircuitBreakerConnection>> {
        let config = ConnectionConfig::from_env()?;
        let cb_config = CircuitBreakerConfig::from_env();
        Self::create_with_circuit_breaker(&config, cb_config).await
    }

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
    pub async fn create_with_cb_and_fallback(
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
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_in_memory() {
        let conn = ConnectionFactory::create_in_memory().await.unwrap();

        let health = conn.health_check().await.unwrap();
        assert!(health.is_healthy);
        assert_eq!(health.mode, ConnectionMode::LocalOnly);
    }

    #[tokio::test]
    async fn test_create_local() {
        let config = ConnectionConfig {
            mode: ConnectionMode::LocalOnly,
            local_path: Some(std::path::PathBuf::from(":memory:")),
            ..Default::default()
        };

        let conn = ConnectionFactory::create_local(&config).await.unwrap();

        let health = conn.health_check().await.unwrap();
        assert!(health.is_healthy);
    }

    #[tokio::test]
    async fn test_config_validation() {
        // Missing local path for replica mode
        let config = ConnectionConfig {
            mode: ConnectionMode::EmbeddedReplica,
            remote_url: "test".to_string(),
            local_path: None,
            ..Default::default()
        };

        let result = ConnectionFactory::create(&config).await;
        assert!(result.is_err());
    }
}
