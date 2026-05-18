//! Local-only connection implementation

use async_trait::async_trait;
use libsql::Database;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::connection::{ConnectionHealth, ConnectionMode, DatabaseConnection};

/// Local-only `SQLite` connection
///
/// This connection type provides:
/// - Pure local `SQLite` database (no remote sync)
/// - Lowest latency (no network overhead)
/// - No cloud dependency
///
/// # Example
///
/// ```rust,no_run
/// use freshcredit_libsql_common::connections::LocalConnection;
/// use freshcredit_libsql_common::DatabaseConnection;
///
/// async fn example() -> anyhow::Result<()> {
///     let conn = LocalConnection::connect("/app/data/local.db").await?;
///     let rows = conn.query("SELECT 1", vec![]).await?;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct LocalConnection {
    db: Arc<Database>,
}

impl LocalConnection {
    /// Create a new local connection
    pub fn new(db: Database) -> Self {
        Self { db: Arc::new(db) }
    }

    /// Create a new local connection with path
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn connect(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let db = libsql::Builder::new_local(path.as_ref()).build().await?;
        Ok(Self::new(db))
    }

    /// Create a new in-memory connection
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn in_memory() -> anyhow::Result<Self> {
        let db = libsql::Builder::new_local(":memory:").build().await?;
        Ok(Self::new(db))
    }

    /// Get the underlying database (for advanced operations)
    #[must_use]
    pub fn database(&self) -> Arc<Database> {
        self.db.clone()
    }
}

#[async_trait]
impl DatabaseConnection for LocalConnection {
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
                    mode: ConnectionMode::LocalOnly,
                    latency_ms: latency,
                    is_healthy: true,
                    last_sync_at: None,
                    cache_hit_rate: Some(1.0),
                })
            }
            Err(e) => {
                let latency = start.elapsed().as_millis() as u64;
                tracing::warn!("Local health check failed: {}", e);
                Ok(ConnectionHealth {
                    mode: ConnectionMode::LocalOnly,
                    latency_ms: latency,
                    is_healthy: false,
                    last_sync_at: None,
                    cache_hit_rate: None,
                })
            }
        }
    }

    fn connection_mode(&self) -> ConnectionMode {
        ConnectionMode::LocalOnly
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_connection() {
        let conn = LocalConnection::in_memory().await.unwrap();

        // Test health check
        let health = conn.health_check().await.unwrap();
        assert!(health.is_healthy);
        assert_eq!(health.mode, ConnectionMode::LocalOnly);

        // Test query
        let mut rows = conn.query("SELECT 1 as value", vec![]).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let value: i32 = row.get(0).unwrap();
        assert_eq!(value, 1);

        // Test database() accessor
        let db = conn.database();
        let c = db.connect().unwrap();
        let mut rows = c.query("SELECT 2", ()).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let value: i32 = row.get(0).unwrap();
        assert_eq!(value, 2);
    }

    #[tokio::test]
    async fn test_execute() {
        // Create a temp file for the database to persist across connections
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("test_local_{}.db", std::process::id()));

        // Clean up before test
        let _ = std::fs::remove_file(&db_path);

        let conn = LocalConnection::connect(&db_path).await.unwrap();

        // Create table
        conn.execute(
            "CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)",
            vec![],
        )
        .await
        .unwrap();

        // Insert
        let affected = conn
            .execute(
                "INSERT INTO test (name) VALUES (?)",
                vec![libsql::Value::Text("test".to_string())],
            )
            .await
            .unwrap();
        assert_eq!(affected, 1);

        // Verify insertion
        let mut rows = conn.query("SELECT name FROM test", vec![]).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let name: String = row.get(0).unwrap();
        assert_eq!(name, "test");

        // Clean up
        let _ = std::fs::remove_file(&db_path);
    }
}
