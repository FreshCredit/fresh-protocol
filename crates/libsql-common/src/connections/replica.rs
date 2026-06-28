//! Embedded replica connection implementation

use async_trait::async_trait;
use libsql::Database;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

// TAG: surface=database owner=platform-team rule=DB-001
use crate::connection::{ConnectionHealth, ConnectionMode, DatabaseConnection};

// TAG: surface=database owner=platform-team rule=DB-001
/// Embedded replica connection with background sync
///
/// This connection type provides:
/// - Local `SQLite` database for low-latency reads
/// - Automatic background synchronization with Turso
/// - Offline capability (reads work when disconnected)
///
/// # Example
///
/// ```rust,no_run
/// use freshcredit_libsql_common::connections::ReplicaConnection;
/// use freshcredit_libsql_common::DatabaseConnection;
///
/// async fn example() -> anyhow::Result<()> {
///     let conn = ReplicaConnection::connect(
///         "/app/data/local.db",
///         "libsql://my-db.turso.io",
///         "my-token",
///         Some(60), // sync interval
///     ).await?;
///
///     let rows = conn.query("SELECT 1", vec![]).await?;
///     Ok(())
/// }
/// ```
// TAG: surface=database owner=platform-team rule=GENERAL-001
#[derive(Debug)]
pub struct ReplicaConnection {
    db: Arc<Database>,
    last_sync: Arc<RwLock<Option<chrono::DateTime<chrono::Utc>>>>,
    sync_interval_secs: Option<u64>,
    background_sync_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl ReplicaConnection {
    /// Create a new replica connection
    pub fn new(db: Database, sync_interval_secs: Option<u64>) -> Self {
        let conn = Self {
            db: Arc::new(db),
            last_sync: Arc::new(RwLock::new(None)),
            sync_interval_secs,
            background_sync_handle: Arc::new(RwLock::new(None)),
        };

        // Start background sync task if interval is set
        if let Some(interval) = sync_interval_secs {
            conn.start_background_sync(interval);
        }

        conn
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Create a new replica connection with all parameters
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn connect(
        local_path: impl AsRef<Path>,
        remote_url: &str,
        auth_token: &str,
        sync_interval_secs: Option<u64>,
    ) -> anyhow::Result<Self> {
        let db = libsql::Builder::new_remote_replica(
            local_path.as_ref(),
            remote_url.to_string(),
            auth_token.to_string(),
        )
        .build()
        .await?;

        // Initial sync
        db.sync().await?;

        Ok(Self::new(db, sync_interval_secs))
    }

    /// Start background sync task
    /// # Errors
    ///
    // TAG: surface=database owner=platform-team rule=GENERAL-001
    /// Returns an error if the operation fails.
    fn start_background_sync(&self, interval_secs: u64) {
        let db = self.db.clone();
        let last_sync = Arc::clone(&self.last_sync);

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                match db.sync().await {
                    Ok(_) => {
                        *last_sync.write().await = Some(chrono::Utc::now());
                        tracing::debug!("Background sync completed");
                    }
                    Err(e) => {
                        tracing::warn!("Background sync failed: {}", e);
                    }
                }
            }
        });

        // Store handle (in a real implementation, you'd want to handle this properly)
        let _ = self.background_sync_handle.try_write().map(|mut guard| {
            *guard = Some(handle);
        });
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Explicit sync on demand
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn sync(&self) -> anyhow::Result<()> {
        self.db.sync().await?;
        *self.last_sync.write().await = Some(chrono::Utc::now());
        Ok(())
    }

    /// Get the last sync time
    pub async fn last_sync(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        *self.last_sync.read().await
    }

    /// Get the sync interval
    #[must_use]
    pub const fn sync_interval_secs(&self) -> Option<u64> {
        self.sync_interval_secs
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get the underlying database (for advanced operations)
    #[must_use]
    pub fn database(&self) -> Arc<Database> {
        self.db.clone()
    }
}

#[async_trait]
impl DatabaseConnection for ReplicaConnection {
    async fn query(&self, sql: &str, params: Vec<libsql::Value>) -> anyhow::Result<libsql::Rows> {
        // Reads are served from local replica (low latency)
        let conn = self.db.connect()?;
        Ok(conn.query(sql, params).await?)
    }

    async fn execute(&self, sql: &str, params: Vec<libsql::Value>) -> anyhow::Result<u64> {
        // Writes go to remote and sync back
        let conn = self.db.connect()?;
        let result = conn.execute(sql, params).await?;

        // Trigger sync after write for consistency
        if let Err(e) = self.db.sync().await {
            tracing::warn!("Post-write sync failed: {}", e);
        } else {
            let mut sync = self.last_sync.write().await;
            *sync = Some(chrono::Utc::now());
        }

        // TAG: surface=database owner=platform-team rule=GENERAL-001
        Ok(result)
    }

    async fn execute_batch(&self, sql: &str) -> anyhow::Result<()> {
        let conn = self.db.connect()?;
        let _ = conn.execute_batch(sql).await?;

        // Sync after batch
        if let Err(e) = self.db.sync().await {
            tracing::warn!("Post-batch sync failed: {}", e);
        } else {
            let mut sync = self.last_sync.write().await;
            *sync = Some(chrono::Utc::now());
        }

        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    async fn health_check(&self) -> anyhow::Result<ConnectionHealth> {
        let start = Instant::now();
        let conn = self.db.connect()?;

        match conn.query("SELECT 1", ()).await {
            Ok(_) => {
                let latency = start.elapsed().as_millis() as u64;
                let last_sync = *self.last_sync.read().await;

                Ok(ConnectionHealth {
                    mode: ConnectionMode::EmbeddedReplica,
                    latency_ms: latency,
                    is_healthy: true,
                    last_sync_at: last_sync,
                    // TAG: surface=database owner=platform-team rule=DB-001
                    cache_hit_rate: Some(1.0), // Local reads = 100% cache hit
                })
            }
            Err(e) => {
                let latency = start.elapsed().as_millis() as u64;
                tracing::warn!("Replica health check failed: {}", e);
                Ok(ConnectionHealth {
                    mode: ConnectionMode::EmbeddedReplica,
                    latency_ms: latency,
                    is_healthy: false,
                    last_sync_at: None,
                    cache_hit_rate: None,
                })
            }
        }
    }

    fn connection_mode(&self) -> ConnectionMode {
        ConnectionMode::EmbeddedReplica
    }

    async fn sync(&self) -> anyhow::Result<()> {
        self.sync().await
    }
}

impl Drop for ReplicaConnection {
    fn drop(&mut self) {
        // Cancel background sync task
        if let Ok(guard) = self.background_sync_handle.try_read() {
            if let Some(ref handle) = *guard {
                handle.abort();
            }
        }
    }
}
