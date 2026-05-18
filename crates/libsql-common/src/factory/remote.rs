use std::sync::Arc;

use crate::connection::{ConnectionConfig, DatabaseConnection};
use crate::connections::RemoteConnection;

impl super::ConnectionFactory {
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
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn create_remote(
        config: &ConnectionConfig,
    ) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let conn = RemoteConnection::connect(&config.remote_url, &config.auth_token).await?;
        Ok(Arc::new(conn))
    }

    /// Create a remote connection with explicit URL and token
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn create_remote_with_url(
        url: &str,
        token: &str,
    ) -> anyhow::Result<Arc<dyn DatabaseConnection>> {
        let conn = RemoteConnection::connect(url, token).await?;
        Ok(Arc::new(conn))
    }
}
