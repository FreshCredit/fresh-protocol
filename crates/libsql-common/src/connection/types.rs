use async_trait::async_trait;
use std::path::PathBuf;

/// Connection mode for libSQL
///
/// # Feature Flags
///
/// - `DirectRemote` and `LocalOnly` are always available (default features)
/// - `EmbeddedReplica` and `Adaptive` require the `embedded-replica` feature
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    /// Direct remote connection to Turso via HTTP
    ///
    /// **Always available** - no feature flag required
    DirectRemote,

    /// Embedded replica with local cache and background sync
    ///
    /// **Requires `embedded-replica` feature**
    #[cfg(feature = "embedded-replica")]
    EmbeddedReplica,

    /// Local-only `SQLite` database
    ///
    /// **Always available** - no feature flag required
    LocalOnly,

    /// Adaptive mode that switches between remote and replica
    ///
    /// **Requires `embedded-replica` feature**
    #[cfg(feature = "embedded-replica")]
    Adaptive,
}

/// Read consistency level for replica mode
///
/// **Requires `embedded-replica` feature**
#[cfg(feature = "embedded-replica")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadConsistency {
    /// Read from local replica (may be stale)
    Eventual,
    /// Read from remote (strong consistency)
    Strong,
    /// Adaptive based on query type
    Adaptive,
}

/// Connection health information
#[derive(Debug, Clone)]
pub struct ConnectionHealth {
    /// Current connection mode
    pub mode: ConnectionMode,
    /// Last measured latency in milliseconds
    pub latency_ms: u64,
    /// Whether the connection is healthy
    pub is_healthy: bool,
    /// Last successful sync time (for replica mode)
    pub last_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Cache hit rate 0.0-1.0 (for replica mode)
    pub cache_hit_rate: Option<f64>,
}

/// Configuration for database connection
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Primary connection mode
    pub mode: ConnectionMode,
    /// Remote URL (Turso)
    pub remote_url: String,
    /// Auth token
    pub auth_token: String,
    /// Local path for embedded replica or local mode
    pub local_path: Option<PathBuf>,
    /// Enable automatic fallback to secondary mode
    pub enable_fallback: bool,
    /// Sync interval for replica mode (seconds)
    pub sync_interval_secs: Option<u64>,
    /// Read consistency level for replica mode
    ///
    /// Only available when `embedded-replica` feature is enabled
    #[cfg(feature = "embedded-replica")]
    pub read_consistency: ReadConsistency,
    /// Connection timeout in seconds
    pub timeout_secs: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
}

/// Unified database connection interface
///
/// This trait abstracts over direct remote and embedded replica connections,
/// providing a consistent API regardless of the underlying connection mode.
#[async_trait]
pub trait DatabaseConnection: Send + Sync {
    /// Execute a query and return rows
    async fn query(&self, sql: &str, params: Vec<libsql::Value>) -> anyhow::Result<libsql::Rows>;

    /// Execute a statement and return affected rows count
    async fn execute(&self, sql: &str, params: Vec<libsql::Value>) -> anyhow::Result<u64>;

    /// Execute a batch of statements
    async fn execute_batch(&self, sql: &str) -> anyhow::Result<()>;

    /// Check connection health
    async fn health_check(&self) -> anyhow::Result<ConnectionHealth>;

    /// Get connection mode
    fn connection_mode(&self) -> ConnectionMode;

    /// Sync with remote (for replica mode, no-op for remote)
    async fn sync(&self) -> anyhow::Result<()> {
        // Default implementation is no-op
        Ok(())
    }
}

/// Extension trait for connection utilities
#[async_trait]
pub trait DatabaseConnectionExt: DatabaseConnection {
    /// Execute a query that returns a single optional row
    ///
    /// The caller is responsible for mapping the row to their type.
    async fn query_optional(
        &self,
        sql: &str,
        params: Vec<libsql::Value>,
    ) -> anyhow::Result<Option<libsql::Row>> {
        let mut rows = self.query(sql, params).await?;
        Ok(rows.next().await?)
    }

    /// Execute a query that returns exactly one row
    ///
    /// The caller is responsible for mapping the row to their type.
    async fn query_one(
        &self,
        sql: &str,
        params: Vec<libsql::Value>,
    ) -> anyhow::Result<libsql::Row> {
        let mut rows = self.query(sql, params).await?;
        (rows.next().await?).map_or_else(|| Err(anyhow::anyhow!("Query returned no rows")), Ok)
    }
}
