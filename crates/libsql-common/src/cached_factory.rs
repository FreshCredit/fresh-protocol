//! Cached connection factory for database-per-user architecture
//!
//! Optimized for Cloud Run: stateless, ephemeral, connection reuse across requests.
//!
//! This factory maintains an LRU cache of database connections to minimize
//! connection establishment overhead for frequently accessed users.
//!
//! # Example
//!
//! ```rust,no_run
//! use freshcredit_libsql_common::cached_factory::{CachedConnectionFactory, CachedFactoryConfig};
//! use std::time::Duration;
//!
//! async fn example() -> anyhow::Result<()> {
//!     let config = CachedFactoryConfig {
//!         max_connections: 1000,
//!         ttl: Duration::from_secs(300),
//!         ..Default::default()
//!     };
//!
//!     let factory = CachedConnectionFactory::new(config, "auth_token".to_string());
//!     let db = factory.get("user-123").await?;
//!
//!     // Use the database
//!     let conn = db.connect()?;
//!     let rows = conn.query("SELECT 1", ()).await?;
//!
//!     Ok(())
//! }
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use libsql::Database;
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tracing::{debug, info};

use crate::connection_factory::{with_retry, RetryConfig};
use crate::url_builder::TursoUrlBuilder;

/// Configuration for cached connection factory
#[derive(Debug, Clone)]
pub struct CachedFactoryConfig {
    /// Maximum number of cached connections
    pub max_connections: usize,
    /// TTL for cached connections
    pub ttl: Duration,
    /// Cleanup interval
    pub cleanup_interval: Duration,
}

impl Default for CachedFactoryConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            ttl: Duration::from_secs(300), // 5 minutes
            cleanup_interval: Duration::from_secs(60),
        }
    }
}

impl CachedFactoryConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            max_connections: std::env::var("DB_CACHE_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            ttl: Duration::from_secs(
                std::env::var("DB_CACHE_TTL_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(300)
            ),
            cleanup_interval: Duration::from_secs(
                std::env::var("DB_CACHE_CLEANUP_INTERVAL_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(60)
            ),
        }
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    /// Total number of cached connections
    pub total_connections: usize,
    /// Cache hit count (cumulative)
    pub hits: u64,
    /// Cache miss count (cumulative)
    pub misses: u64,
}

impl CacheStats {
    /// Calculate hit rate (0.0 - 1.0)
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Internal entry for cache tracking
type CacheEntry = (Arc<Database>, Instant);

/// Cached connection factory for per-user databases
pub struct CachedConnectionFactory {
    /// LRU cache: user_id -> (Database, last_used)
    connections: Arc<DashMap<String, CacheEntry>>,
    /// Configuration
    config: CachedFactoryConfig,
    /// URL builder for consistent naming
    url_builder: TursoUrlBuilder,
    /// Auth token for database access
    auth_token: String,
    /// Cache statistics
    stats: Arc<std::sync::atomic::AtomicU64>,
    /// Cleanup task handle
    cleanup_handle: Arc<AsyncMutex<Option<JoinHandle<()>>>>,
}

impl std::fmt::Debug for CachedConnectionFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedConnectionFactory")
            .field("config", &self.config)
            .field("connections_count", &self.connections.len())
            .field("url_builder", &self.url_builder)
            .finish()
    }
}

impl CachedConnectionFactory {
    /// Create new cached factory
    ///
    /// # Arguments
    ///
    /// * `config` - Factory configuration (max connections, TTL, etc.)
    /// * `auth_token` - Turso authentication token
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use freshcredit_libsql_common::cached_factory::{CachedConnectionFactory, CachedFactoryConfig};
    ///
    /// let factory = CachedConnectionFactory::new(
    ///     CachedFactoryConfig::default(),
    ///     "your-auth-token".to_string()
    /// );
    /// ```
    pub fn new(config: CachedFactoryConfig, auth_token: String) -> Self {
        let factory = Self {
            connections: Arc::new(DashMap::new()),
            config,
            url_builder: TursoUrlBuilder::from_env(),
            auth_token,
            stats: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cleanup_handle: Arc::new(AsyncMutex::new(None)),
        };

        // Start background cleanup task
        factory.start_cleanup_task();

        factory
    }

    /// Create from environment variables
    ///
    /// Uses `TURSO_AUTH_TOKEN` from environment for authentication.
    /// Configuration loaded from `DB_CACHE_*` environment variables.
    ///
    /// # Errors
    ///
    /// Returns error if `TURSO_AUTH_TOKEN` is not set.
    pub fn from_env() -> anyhow::Result<Self> {
        let auth_token = std::env::var("TURSO_AUTH_TOKEN")
            .map_err(|_| anyhow::anyhow!("TURSO_AUTH_TOKEN not set"))?;

        let config = CachedFactoryConfig::from_env();

        Ok(Self::new(config, auth_token))
    }

    /// Get or create a database connection for a user
    ///
    /// Returns cached connection if available and not expired,
    /// otherwise creates a new connection.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The user identifier
    ///
    /// # Errors
    ///
    /// Returns error if connection creation fails.
    pub async fn get(&self, user_id: &str) -> anyhow::Result<Arc<Database>> {
        // Check cache first
        if let Some(mut entry) = self.connections.get_mut(user_id) {
            let (db, last_used) = entry.value_mut();
            if last_used.elapsed() < self.config.ttl {
                debug!(user_id = %user_id, "Cache hit");
                // Update last used time
                *last_used = Instant::now();

                // Update stats (approximate)
                self.stats.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                return Ok(db.clone());
            }
        }

        // Cache miss - create new connection
        debug!(user_id = %user_id, "Cache miss, creating connection");
        let db = self.create_connection(user_id).await?;
        let db_arc = Arc::new(db);

        // Store in cache
        self.connections.insert(
            user_id.to_string(),
            (db_arc.clone(), Instant::now())
        );

        // Update stats
        self.stats.fetch_add(1 << 32, std::sync::atomic::Ordering::Relaxed);

        // Evict oldest if over limit
        if self.connections.len() > self.config.max_connections {
            self.evict_oldest().await;
        }

        Ok(db_arc)
    }

    /// Create a new connection to user's database
    async fn create_connection(&self, user_id: &str) -> anyhow::Result<Database> {
        let url = self.url_builder.user_database_url(user_id);
        let https_url = url.replace("libsql://", "https://");

        let retry_config = RetryConfig::default();

        with_retry(&retry_config, || {
            let url = https_url.clone();
            let token = self.auth_token.clone();
            async move {
                libsql::Builder::new_remote(url, token)
                    .build()
                    .await
            }
        }).await.map_err(|e| anyhow::anyhow!("Failed to create connection: {}", e))
    }

    /// Evict oldest connection from cache
    async fn evict_oldest(&self) {
        let oldest = self.connections
            .iter()
            .min_by_key(|entry| entry.value().1)
            .map(|entry| entry.key().clone());

        if let Some(user_id) = oldest {
            debug!(user_id = %user_id, "Evicting oldest connection");
            self.connections.remove(&user_id);
        }
    }

    /// Start background cleanup task
    fn start_cleanup_task(&self) {
        let connections = Arc::clone(&self.connections);
        let ttl = self.config.ttl;
        let interval_duration = self.config.cleanup_interval;

        let handle = tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                interval.tick().await;

                let now = Instant::now();
                let to_remove: Vec<String> = connections
                    .iter()
                    .filter(|entry| now.duration_since(entry.value().1) > ttl)
                    .map(|entry| entry.key().clone())
                    .collect();

                let removed_count = to_remove.len();
                for user_id in to_remove {
                    debug!(user_id = %user_id, "Cleaning up expired connection");
                    connections.remove(&user_id);
                }

                if removed_count > 0 {
                    info!(
                        removed = removed_count,
                        remaining = connections.len(),
                        "Cache cleanup complete"
                    );
                }
            }
        });

        let mut guard = self.cleanup_handle.lock().await;
        *guard = Some(handle);
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let stats_val = self.stats.load(std::sync::atomic::Ordering::Relaxed);
        CacheStats {
            total_connections: self.connections.len(),
            hits: stats_val & 0xFFFFFFFF,
            misses: stats_val >> 32,
        }
    }

    /// Remove a specific user from cache
    ///
    /// Useful for logout or session invalidation scenarios.
    pub fn invalidate(&self, user_id: &str) {
        self.connections.remove(user_id);
        debug!(user_id = %user_id, "Invalidated cache entry");
    }

    /// Clear all cached connections
    pub fn clear(&self) {
        self.connections.clear();
        info!("Cache cleared");
    }
}

impl Drop for CachedConnectionFactory {
    fn drop(&mut self) {
        // Cancel background cleanup task
        // Note: We can't use async mutex in drop, so we use try_lock
        // If the mutex is locked, the task will be cleaned up when the runtime drops it
        if let Ok(guard) = self.cleanup_handle.try_lock() {
            if let Some(ref handle) = *guard {
                handle.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_factory_config_default() {
        let config = CachedFactoryConfig::default();
        assert_eq!(config.max_connections, 1000);
        assert_eq!(config.ttl, Duration::from_secs(300));
        assert_eq!(config.cleanup_interval, Duration::from_secs(60));
    }

    #[test]
    fn test_cache_stats_hit_rate() {
        let stats = CacheStats {
            total_connections: 10,
            hits: 90,
            misses: 10,
        };
        assert!((stats.hit_rate() - 0.9).abs() < 0.001);

        let empty_stats = CacheStats {
            total_connections: 0,
            hits: 0,
            misses: 0,
        };
        assert_eq!(empty_stats.hit_rate(), 0.0);
    }
}
