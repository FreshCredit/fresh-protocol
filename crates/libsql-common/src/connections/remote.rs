//! Direct remote connection implementation

use async_trait::async_trait;
use libsql::Database;
use std::sync::Arc;
use std::time::Instant;

use crate::connection::{
    ConnectionHealth,
    ConnectionMode,
    DatabaseConnection,
};

/// Direct remote connection to Turso
///
/// This connection type provides:
/// - Direct HTTP/HTTPS connection to Turso edge
/// - Strong consistency for all reads
/// - No local state or caching
///
/// # Example
///
/// ```rust,no_run
/// use freshcredit_libsql_common::connections::RemoteConnection;
/// use freshcredit_libsql_common::DatabaseConnection;
///
/// async fn example() -> anyhow::Result<()> {
///     let db = libsql::Builder::new_remote(
///         "libsql://my-db.turso.io".to_string(),
///         "my-token".to_string(),
///     ).build().await?;
///
///     let conn = RemoteConnection::new(db);
///     let rows = conn.query("SELECT 1", vec![]).await?;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct RemoteConnection {
    db: Arc<Database>,
}

impl RemoteConnection {
    /// Create a new remote connection
    pub fn new(db: Database) -> Self {
        Self { db: Arc::new(db) }
    }

    /// Create a new remote connection with URL and token
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn connect(url: &str, token: &str) -> anyhow::Result<Self> {
        let db = libsql::Builder::new_remote(url.to_string(), token.to_string())
            .build()
            .await?;
        Ok(Self::new(db))
    }

    /// Get the underlying database (for advanced operations)
    #[must_use]
    pub fn database(&self) -> Arc<Database> {
        self.db.clone()
    }
}

#[async_trait]
impl DatabaseConnection for RemoteConnection {
    async fn query(&self, sql: &str, params: Vec<libsql::Value>) -> anyhow::Result<libsql::Rows> {
        let conn = self.db.connect()?;
        Ok(conn.query(sql, params).await?)
    }

    async fn execute(&self, sql: &str, params: Vec<libsql::Value>) -> anyhow::Result<u64> {
        let conn = self.db.connect()?;
        Ok(conn.execute(sql, params).await?)
    }

    async fn execute_batch(&self, sql: &str) -> anyhow::Result<()> {
        let conn = self.db.connect()?;
        let _ = conn.execute_batch(sql).await?;
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    async fn health_check(&self) -> anyhow::Result<ConnectionHealth> {
        let start = Instant::now();
        let conn = self.db.connect()?;

        match conn.query("SELECT 1", ()).await {
            Ok(_) => {
                let latency = start.elapsed().as_millis() as u64;
                Ok(ConnectionHealth {
                    mode: ConnectionMode::DirectRemote,
                    latency_ms: latency,
                    is_healthy: true,
                    last_sync_at: None,
                    cache_hit_rate: None,
                })
            }
            Err(e) => {
                let latency = start.elapsed().as_millis() as u64;
                tracing::warn!("Remote health check failed: {}", e);
                Ok(ConnectionHealth {
                    mode: ConnectionMode::DirectRemote,
                    latency_ms: latency,
                    is_healthy: false,
                    last_sync_at: None,
                    cache_hit_rate: None,
                })
            }
        }
    }

    fn connection_mode(&self) -> ConnectionMode {
        ConnectionMode::DirectRemote
    }
}
