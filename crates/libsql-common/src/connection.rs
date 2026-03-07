//! Unified database connection interface for libSQL
//!
//! This module provides a trait-based abstraction over different libSQL connection modes:
//! - Direct remote (HTTP to Turso)
//! - Embedded replica (local + background sync)
//! - Local only (file-based SQLite)
//!
//! # Example
//!
//! ```rust,no_run
//! use freshcredit_libsql_common::{ConnectionFactory, ConnectionConfig, ConnectionMode};
//!
//! async fn example() -> anyhow::Result<()> {
//!     let config = ConnectionConfig {
//!         mode: ConnectionMode::EmbeddedReplica,
//!         remote_url: "libsql://my-db.turso.io".to_string(),
//!         auth_token: "token".to_string(),
//!         local_path: Some("/app/data/local.db".into()),
//!         ..Default::default()
//!     };
//!     
//!     let db = ConnectionFactory::create(&config).await?;
//!     let rows = db.query("SELECT 1", vec![]).await?;
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use std::fmt;
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
    
    /// Local-only SQLite database
    ///
    /// **Always available** - no feature flag required
    LocalOnly,
    
    /// Adaptive mode that switches between remote and replica
    ///
    /// **Requires `embedded-replica` feature**
    #[cfg(feature = "embedded-replica")]
    Adaptive,
}

impl fmt::Display for ConnectionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionMode::DirectRemote => write!(f, "direct-remote"),
            #[cfg(feature = "embedded-replica")]
            ConnectionMode::EmbeddedReplica => write!(f, "embedded-replica"),
            ConnectionMode::LocalOnly => write!(f, "local-only"),
            #[cfg(feature = "embedded-replica")]
            ConnectionMode::Adaptive => write!(f, "adaptive"),
        }
    }
}

impl std::str::FromStr for ConnectionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "remote" | "direct-remote" => Ok(ConnectionMode::DirectRemote),
            "local" | "local-only" => Ok(ConnectionMode::LocalOnly),
            
            #[cfg(feature = "embedded-replica")]
            "replica" | "embedded-replica" => Ok(ConnectionMode::EmbeddedReplica),
            
            #[cfg(feature = "embedded-replica")]
            "adaptive" => Ok(ConnectionMode::Adaptive),
            
            #[cfg(not(feature = "embedded-replica"))]
            "replica" | "embedded-replica" | "adaptive" => Err(format!(
                "Connection mode '{}' requires the 'embedded-replica' feature. \
                 Enable it in Cargo.toml: freshcredit-libsql-common = {{ features = [\"embedded-replica\"] }}",
                s
            )),
            
            _ => Err(format!("Unknown connection mode: {s}")),
        }
    }
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

#[cfg(feature = "embedded-replica")]
impl fmt::Display for ReadConsistency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadConsistency::Eventual => write!(f, "eventual"),
            ReadConsistency::Strong => write!(f, "strong"),
            ReadConsistency::Adaptive => write!(f, "adaptive"),
        }
    }
}

#[cfg(feature = "embedded-replica")]
impl std::str::FromStr for ReadConsistency {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "eventual" => Ok(ReadConsistency::Eventual),
            "strong" => Ok(ReadConsistency::Strong),
            "adaptive" => Ok(ReadConsistency::Adaptive),
            _ => Err(format!("Unknown read consistency: {s}")),
        }
    }
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

impl ConnectionHealth {
    /// Create a healthy status
    pub fn healthy(mode: ConnectionMode, latency_ms: u64) -> Self {
        Self {
            mode,
            latency_ms,
            is_healthy: true,
            last_sync_at: None,
            cache_hit_rate: None,
        }
    }

    /// Create an unhealthy status
    pub fn unhealthy(mode: ConnectionMode, latency_ms: u64) -> Self {
        Self {
            mode,
            latency_ms,
            is_healthy: false,
            last_sync_at: None,
            cache_hit_rate: None,
        }
    }
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

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            mode: ConnectionMode::DirectRemote,
            remote_url: String::new(),
            auth_token: String::new(),
            local_path: None,
            enable_fallback: false,
            sync_interval_secs: Some(60),
            #[cfg(feature = "embedded-replica")]
            read_consistency: ReadConsistency::Eventual,
            timeout_secs: 30,
            max_retries: 3,
        }
    }
}

impl ConnectionConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        use std::env;

        let mode = env::var("LIBSQL_MODE")
            .unwrap_or_else(|_| "remote".to_string())
            .parse::<ConnectionMode>()
            .map_err(|e| anyhow::anyhow!(e))?;

        let remote_url = env::var("TURSO_URL")
            .or_else(|_| env::var("LIBSQL_URL"))
            .unwrap_or_default();

        let auth_token = env::var("TURSO_AUTH_TOKEN")
            .or_else(|_| env::var("LIBSQL_AUTH_TOKEN"))
            .unwrap_or_default();

        let local_path = env::var("LIBSQL_LOCAL_PATH")
            .ok()
            .map(PathBuf::from);

        let enable_fallback = env::var("LIBSQL_ENABLE_FALLBACK")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let sync_interval_secs = env::var("LIBSQL_SYNC_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok());

        #[cfg(feature = "embedded-replica")]
        let read_consistency = env::var("LIBSQL_READ_CONSISTENCY")
            .unwrap_or_else(|_| "eventual".to_string())
            .parse::<ReadConsistency>()
            .map_err(|e| anyhow::anyhow!(e))?;

        let timeout_secs = env::var("LIBSQL_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30);

        let max_retries = env::var("LIBSQL_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(3);

        Ok(Self {
            mode,
            remote_url,
            auth_token,
            local_path,
            enable_fallback,
            sync_interval_secs,
            #[cfg(feature = "embedded-replica")]
            read_consistency,
            timeout_secs,
            max_retries,
        })
    }

    /// Validate the configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        match self.mode {
            ConnectionMode::DirectRemote => {
                if self.remote_url.is_empty() {
                    return Err(anyhow::anyhow!(
                        "Remote URL is required for direct-remote mode"
                    ));
                }
            }
            ConnectionMode::LocalOnly => {
                if self.local_path.is_none() {
                    return Err(anyhow::anyhow!(
                        "Local path is required for local-only mode"
                    ));
                }
            }
            #[cfg(feature = "embedded-replica")]
            ConnectionMode::EmbeddedReplica => {
                if self.remote_url.is_empty() {
                    return Err(anyhow::anyhow!(
                        "Remote URL is required for embedded replica mode"
                    ));
                }
                if self.local_path.is_none() {
                    return Err(anyhow::anyhow!(
                        "Local path is required for embedded replica mode"
                    ));
                }
            }
            #[cfg(feature = "embedded-replica")]
            ConnectionMode::Adaptive => {
                if self.remote_url.is_empty() {
                    return Err(anyhow::anyhow!(
                        "Remote URL is required for adaptive mode"
                    ));
                }
            }
        }
        Ok(())
    }
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
        if let Some(row) = rows.next().await? {
            Ok(row)
        } else {
            Err(anyhow::anyhow!("Query returned no rows"))
        }
    }
}

#[async_trait]
impl<T: DatabaseConnection> DatabaseConnectionExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_mode_from_str() {
        assert_eq!(
            "remote".parse::<ConnectionMode>().unwrap(),
            ConnectionMode::DirectRemote
        );
        assert_eq!(
            "replica".parse::<ConnectionMode>().unwrap(),
            ConnectionMode::EmbeddedReplica
        );
        assert_eq!(
            "local".parse::<ConnectionMode>().unwrap(),
            ConnectionMode::LocalOnly
        );
        assert_eq!(
            "adaptive".parse::<ConnectionMode>().unwrap(),
            ConnectionMode::Adaptive
        );
        assert!("unknown".parse::<ConnectionMode>().is_err());
    }

    #[test]
    fn test_read_consistency_from_str() {
        assert_eq!(
            "eventual".parse::<ReadConsistency>().unwrap(),
            ReadConsistency::Eventual
        );
        assert_eq!(
            "strong".parse::<ReadConsistency>().unwrap(),
            ReadConsistency::Strong
        );
        assert_eq!(
            "adaptive".parse::<ReadConsistency>().unwrap(),
            ReadConsistency::Adaptive
        );
    }

    #[test]
    fn test_config_validate() {
        // Remote mode requires URL
        let config = ConnectionConfig {
            mode: ConnectionMode::DirectRemote,
            remote_url: String::new(),
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // Replica mode requires URL and local path
        let config = ConnectionConfig {
            mode: ConnectionMode::EmbeddedReplica,
            remote_url: "test".to_string(),
            local_path: None,
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // Local mode requires path
        let config = ConnectionConfig {
            mode: ConnectionMode::LocalOnly,
            local_path: None,
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // Valid config
        let config = ConnectionConfig {
            mode: ConnectionMode::LocalOnly,
            local_path: Some(PathBuf::from("/tmp/test.db")),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }
}
