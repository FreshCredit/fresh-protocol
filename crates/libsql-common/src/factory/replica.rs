use std::sync::Arc;

use crate::connection::ConnectionConfig;
use crate::connection::DatabaseConnection;
use crate::connections::ReplicaConnection;

impl super::ConnectionFactory {
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
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
}
