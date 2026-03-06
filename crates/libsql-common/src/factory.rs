//! Connection factory for creating database connections
//!
//! This module provides a factory pattern for creating different types of
//! database connections based on configuration.
//!
//! # Example
//!
//! ```rust,no_run
//! use freshcredit_libsql_common::{
//!     ConnectionFactory, ConnectionConfig, ConnectionMode
//! };
//!
//! async fn example() -> anyhow::Result<()> {
//!     let config = ConnectionConfig {
//!         mode: ConnectionMode::EmbeddedReplica,
//!         remote_url: "libsql://my-db.turso.io".to_string(),
//!         auth_token: "token".to_string(),
//!         local_path: Some("/app/data/local.db".into()),
//!         sync_interval_secs: Some(60),
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

use crate::connection::{ConnectionConfig, ConnectionMode, DatabaseConnection};
use crate::connections::{LocalConnection, RemoteConnection, ReplicaConnection};

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
    pub async fn create(config: &ConnectionConfig) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        // Validate configuration first
        config.validate()?;

        match config.mode {
            ConnectionMode::DirectRemote => Self::create_remote(config).await,
            ConnectionMode::EmbeddedReplica => Self::create_replica(config).await,
            ConnectionMode::LocalOnly => Self::create_local(config).await,
            ConnectionMode::Adaptive => {
                // For adaptive mode, we need both remote and replica configs
                // Default to replica as primary with remote as fallback
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
    pub async fn create_remote(config: &ConnectionConfig) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let conn = RemoteConnection::connect(&config.remote_url, &config.auth_token).await?;
        Ok(Arc::new(conn))
    }

    /// Create a remote connection with explicit URL and token
    pub async fn create_remote_with_url(url: &str, token: &str) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
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
    pub async fn create_replica(config: &ConnectionConfig) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
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
    pub async fn create_replica_with_params(
        local_path: impl AsRef<std::path::Path>,
        remote_url: &str,
        auth_token: &str,
        sync_interval_secs: Option<u64>,
    ) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let conn = ReplicaConnection::connect(
            local_path,
            remote_url,
            auth_token,
            sync_interval_secs,
        )
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
    pub async fn create_local(config: &ConnectionConfig) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let local_path = config
            .local_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Local path required for local mode"))?;

        let conn = LocalConnection::connect(local_path).await?;
        Ok(Arc::new(conn))
    }

    /// Create a local-only connection with explicit path
    pub async fn create_local_with_path(path: impl AsRef<std::path::Path>) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
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
                            "Both primary and fallback connections failed: primary={}, fallback={}",
                            e,
                            e2
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
