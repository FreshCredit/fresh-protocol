use std::sync::Arc;

use crate::connection::{ConnectionConfig, DatabaseConnection};
use crate::connections::LocalConnection;

impl super::ConnectionFactory {
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
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn create_in_memory() -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let conn = LocalConnection::in_memory().await?;
        Ok(Arc::new(conn))
    }
}
