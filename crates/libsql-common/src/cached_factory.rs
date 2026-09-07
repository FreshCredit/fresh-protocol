//! Cached connection factory for database-per-user architecture
//!
//! Optimized for Cloud Run: stateless, ephemeral, connection reuse across requests.
//!
//! This factory maintains an LRU cache of database connections to minimize
// TAG: surface=database owner=platform-team rule=DB-001
//! connection establishment overhead for frequently accessed users.
//!
//! # Concurrency: per-key open dedup
//!
//! Concurrent `get`/`get_for` calls for the same user share ONE in-flight
//! open: a per-user async mutex serializes opens per key, and every waiter
//! re-checks the cache after acquiring the lock, so a user is opened at most
//! once per (URL, token, TTL) generation no matter how many requests race.
//! Concurrent calls for *different* users open in parallel.
//!
//! # Eviction and connection lifecycle
//!
//! Cache entries hold `Arc<Database>` handles. Eviction (LRU overflow or TTL
//! expiry, including the background cleanup task) drops only the cache's own
//! strong reference — callers holding clones keep working, and the underlying
//! remote handle (hyper client pool) is released when the last `Arc` drops.
//! Remote `Database` handles are lazy (no handshake at build time) and have no
//! explicit close API, so there is no fd/socket leak beyond the configured
//! LRU bound.
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

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use libsql::Database;
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tracing::{debug, info, warn};

use crate::connection_factory::{with_retry, RetryConfig};
use crate::url_builder::TursoUrlBuilder;

// TAG: surface=database owner=platform-team rule=DB-001
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

// TAG: surface=database owner=platform-team rule=DB-001
impl CachedFactoryConfig {
    /// Create from environment variables
    #[must_use]
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
                    .unwrap_or(300),
            ),
            cleanup_interval: Duration::from_secs(
                std::env::var("DB_CACHE_CLEANUP_INTERVAL_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(60),
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

// TAG: surface=database owner=platform-team rule=DB-001
impl CacheStats {
    /// Calculate hit rate (0.0 - 1.0)
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "hit-rate is a diagnostic ratio; f64 mantissa is more than enough"
    )]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Internal entry for cache tracking: `(connection, last_used, https_url, token)`.
///
/// The URL and token are recorded so a cache hit can be validated against the
/// caller's current credentials — a rotated token or moved database triggers a
/// reopen instead of silently reusing a stale handle.
type CacheEntry = (Arc<Database>, Instant, Arc<str>, Arc<str>);

/// Async opener used by [`CachedConnectionFactory`] to establish a remote
/// database connection: `(database_url, auth_token) -> Database`.
///
/// Injectable so tests can count/latency-instrument opens without network
/// access, and so embedders can supply custom transports.
pub type OpenConnector = Arc<
    dyn Fn(&str, &str) -> Pin<Box<dyn Future<Output = anyhow::Result<Database>> + Send>>
        + Send
        + Sync,
>;

/// Open a remote libSQL database over HTTPS with centralized retry logic.
///
/// This is the SINGLE open path shared by every consumer: the cached factory's
/// default connector, and direct callers such as the app's user-database
/// service. `database_url` must already be the `https://` form (the
/// `libsql://` → `https://` rewrite happens at the call site that owns the
/// URL, so this function never needs to guess).
///
/// Remote builds are lazy: no handshake happens here, so this is cheap enough
/// to call per request when an owned (non-`Clone`) `Database` must be handed out.
///
/// # Errors
///
/// Returns an error if the connection cannot be built within the retry budget.
pub async fn open_remote_database(
    database_url: &str,
    auth_token: &str,
    retry_config: &RetryConfig,
) -> anyhow::Result<Database> {
    let url = database_url.to_string();
    let token = auth_token.to_string();

    with_retry(retry_config, move || {
        let url = url.clone();
        let token = token.clone();
        async move { libsql::Builder::new_remote(url, token).build().await }
    })
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create connection: {e}"))
}

/// Cached connection factory for per-user databases
pub struct CachedConnectionFactory {
    /// LRU cache: `user_id -> (Database, last_used, https_url, token)`
    connections: Arc<DashMap<String, CacheEntry>>,
    /// Per-user open locks: concurrent gets for one user share one in-flight open
    inflight: Arc<DashMap<String, Arc<AsyncMutex<()>>>>,
    /// Configuration
    config: CachedFactoryConfig,
    /// URL builder for consistent naming
    url_builder: TursoUrlBuilder,
    /// Default auth token (used by [`Self::get`])
    auth_token: String,
    /// Injected opener (default: [`open_remote_database`])
    connector: OpenConnector,
    /// Cache statistics
    stats: Arc<std::sync::atomic::AtomicU64>,
    /// Cleanup task handle
    cleanup_handle: Arc<AsyncMutex<Option<JoinHandle<()>>>>,
}

// The manual Debug intentionally omits the auth token (secret) and the
// connector/stats internals.
#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for CachedConnectionFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedConnectionFactory")
            .field("config", &self.config)
            .field("connections_count", &self.connections.len())
            .field("url_builder", &self.url_builder)
            .finish()
    }
}

// TAG: surface=database owner=platform-team rule=DB-001
impl CachedConnectionFactory {
    /// Create new cached factory with the default remote opener.
    ///
    /// # Arguments
    ///
    /// * `config` - Factory configuration (max connections, TTL, etc.)
    /// * `auth_token` - Turso authentication token used by [`Self::get`]
    ///
    /// # Panics
    ///
    /// Does not panic. If called outside a Tokio runtime the background
    /// cleanup task is skipped (entries still expire lazily on access).
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
    #[must_use]
    pub fn new(config: CachedFactoryConfig, auth_token: String) -> Self {
        let retry_config = RetryConfig::from_env();
        let connector: OpenConnector = Arc::new(move |url: &str, token: &str| {
            let url = url.to_string();
            let token = token.to_string();
            let retry_config = retry_config.clone();
            Box::pin(async move { open_remote_database(&url, &token, &retry_config).await })
        });

        Self::with_connector(config, auth_token, connector)
    }

    /// Create a cached factory with an injected opener.
    ///
    /// Used by tests (to count opens without network access) and by embedders
    /// that need a custom transport. Production code should use [`Self::new`].
    #[must_use]
    pub fn with_connector(
        config: CachedFactoryConfig,
        auth_token: String,
        connector: OpenConnector,
    ) -> Self {
        let factory = Self {
            connections: Arc::new(DashMap::new()),
            inflight: Arc::new(DashMap::new()),
            config,
            url_builder: TursoUrlBuilder::from_env(),
            auth_token,
            connector,
            stats: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cleanup_handle: Arc::new(AsyncMutex::new(None)),
        };

        // Start background cleanup task (skipped when no Tokio runtime is
        // available — e.g. synchronous unit tests; TTL still enforced lazily).
        factory.start_cleanup_task();

        factory
    }

    // TAG: surface=database owner=platform-team rule=DB-001
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

    /// Get or create a database connection for a user, using the factory's
    /// default token and deterministically built URL.
    ///
    /// Returns the cached connection if available, fresh, and opened with the
    /// same credentials; otherwise creates a new one.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The user identifier
    ///
    /// # Errors
    ///
    // TAG: surface=database owner=platform-team rule=GENERAL-001
    /// Returns error if connection creation fails.
    pub async fn get(&self, user_id: &str) -> anyhow::Result<Arc<Database>> {
        let url = self
            .url_builder
            .user_database_url(user_id)
            .replace("libsql://", "https://");
        self.get_for(user_id, &url, &self.auth_token).await
    }

    /// Get or create a database connection for a user with explicit
    /// connection material (HTTPS URL + auth token).
    ///
    /// This is the primitive behind [`Self::get`]; services that learn the
    /// real URL/token from provisioning (e.g. region-pinned hostnames or
    /// per-user database-scoped tokens) should call this so cache hits are
    /// validated against the exact credentials they would use.
    ///
    /// Concurrency: concurrent calls for the same `user_id` share one
    /// in-flight open (per-key dedup); different users open in parallel.
    ///
    /// # Errors
    ///
    /// Returns error if connection creation fails.
    pub async fn get_for(
        &self,
        user_id: &str,
        database_url: &str,
        auth_token: &str,
    ) -> anyhow::Result<Arc<Database>> {
        // Fast path: fresh entry with matching credentials.
        if let Some(db) = self.lookup_fresh(user_id, database_url, auth_token) {
            debug!(user_id = %user_id, "Cache hit");
            self.bump_stats(true);
            return Ok(db);
        }

        // Cache miss — serialize opens per user so concurrent requests share
        // one in-flight connection establishment.
        let key_lock = {
            let entry_ref = self
                .inflight
                .entry(user_id.to_string())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())));
            let lock = Arc::clone(entry_ref.value());
            drop(entry_ref);
            lock
        };
        let guard = key_lock.lock().await;

        // Re-check after acquiring the lock: another request may have opened
        // the connection while we were waiting.
        if let Some(db) = self.lookup_fresh(user_id, database_url, auth_token) {
            debug!(user_id = %user_id, "Cache hit after wait");
            drop(guard);
            self.release_inflight(user_id, &key_lock);
            self.bump_stats(true);
            return Ok(db);
        }

        // Open through the single shared open path.
        debug!(user_id = %user_id, "Cache miss, creating connection");
        let db = (self.connector)(database_url, auth_token).await;
        let db = match db {
            Ok(db) => db,
            Err(e) => {
                drop(guard);
                self.release_inflight(user_id, &key_lock);
                return Err(e);
            }
        };

        let db_arc = Arc::new(db);
        self.connections.insert(
            user_id.to_string(),
            (
                Arc::clone(&db_arc),
                Instant::now(),
                database_url.into(),
                auth_token.into(),
            ),
        );

        // TAG: surface=database owner=platform-team rule=DB-001
        // Update stats
        self.bump_stats(false);

        // Evict oldest if over limit
        if self.connections.len() > self.config.max_connections {
            self.evict_oldest();
        }

        drop(guard);
        self.release_inflight(user_id, &key_lock);

        Ok(db_arc)
    }

    /// Return the cached connection for `user_id` when it is fresh and was
    /// opened with exactly these credentials. Refreshes the last-used
    /// timestamp for LRU/TTL accounting on a hit (best-effort: the cleanup
    /// task may race us and expire the entry, which is fine — the returned
    /// handle stays valid either way).
    fn lookup_fresh(
        &self,
        user_id: &str,
        database_url: &str,
        auth_token: &str,
    ) -> Option<Arc<Database>> {
        let entry = self.connections.get(user_id)?;
        let (db, last_used, url, token) = entry.value();
        if last_used.elapsed() >= self.config.ttl
            || url.as_ref() != database_url
            || token.as_ref() != auth_token
        {
            return None;
        }
        let db = Arc::clone(db);
        drop(entry);
        if let Some(mut entry) = self.connections.get_mut(user_id) {
            entry.1 = Instant::now();
        }
        Some(db)
    }

    /// Increment hit (bit 0..32) or miss (bit 32..64) counters.
    fn bump_stats(&self, hit: bool) {
        let delta: u64 = if hit { 1 } else { 1 << 32 };
        self.stats
            .fetch_add(delta, std::sync::atomic::Ordering::Relaxed);
    }

    /// Remove this caller's per-key open lock, but only if the map still
    /// points at OUR mutex (another caller may already have replaced it).
    fn release_inflight(&self, user_id: &str, lock: &Arc<AsyncMutex<()>>) {
        if let Some((_, removed)) = self
            .inflight
            .remove_if(user_id, |_, v| Arc::ptr_eq(v, lock))
        {
            drop(removed);
        }
    }

    /// Evict oldest connection from cache (by last-used time).
    fn evict_oldest(&self) {
        let oldest = self
            .connections
            .iter()
            .min_by_key(|entry| entry.value().1)
            .map(|entry| entry.key().clone());

        // TAG: surface=database owner=platform-team rule=DB-001
        if let Some(user_id) = oldest {
            debug!(user_id = %user_id, "Evicting oldest connection");
            self.connections.remove(&user_id);
        }
    }

    /// Start background cleanup task
    fn start_cleanup_task(&self) {
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            debug!(
                "No Tokio runtime available; skipping cache cleanup task (TTL still enforced lazily)"
            );
            return;
        };

        let connections = Arc::clone(&self.connections);
        let ttl = self.config.ttl;
        let interval_duration = self.config.cleanup_interval;

        let task = handle.spawn(async move {
            let mut interval = interval(interval_duration);

            // P10-R4: intentionally unbounded service loop — TTL cleanup ticks until the spawned task is aborted at shutdown.
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

        if let Ok(mut guard) = self.cleanup_handle.try_lock() {
            *guard = Some(task);
        } else {
            // Extremely unlikely (mutex is private and uncontended here);
            // abort rather than leak a detached task.
            task.abort();
            warn!("Could not store cache cleanup handle; task aborted");
        }
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Get cache statistics
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        let stats_val = self.stats.load(std::sync::atomic::Ordering::Relaxed);
        CacheStats {
            total_connections: self.connections.len(),
            hits: stats_val & 0xFFFF_FFFF,
            misses: stats_val >> 32,
        }
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Remove a specific user from cache
    ///
    /// Useful for logout or session invalidation scenarios. Drops the cache's
    /// reference; live callers holding `Arc<Database>` clones keep their handle.
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
// TAG: surface=database owner=platform-team rule=GENERAL-001

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Test connector that counts opens and serves in-memory databases, with
    /// a small delay to force request overlap.
    fn counting_connector(opens: Arc<AtomicUsize>) -> OpenConnector {
        Arc::new(move |_url: &str, _token: &str| {
            let opens = Arc::clone(&opens);
            Box::pin(async move {
                opens.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(5)).await;
                let db = libsql::Builder::new_local(":memory:").build().await?;
                Ok(db)
            })
        })
    }

    fn test_config() -> CachedFactoryConfig {
        CachedFactoryConfig {
            max_connections: 100,
            ttl: Duration::from_secs(300),
            cleanup_interval: Duration::from_secs(60),
        }
    }

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
        assert!(empty_stats.hit_rate() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_concurrent_gets_deduplicate_opens() {
        let opens = Arc::new(AtomicUsize::new(0));
        let factory = Arc::new(CachedConnectionFactory::with_connector(
            test_config(),
            String::new(),
            counting_connector(Arc::clone(&opens)),
        ));

        let users = ["user-1", "user-2", "user-3", "user-4", "user-5"];
        let mut handles = Vec::new();
        for user in users.iter().cycle().take(50) {
            let factory = Arc::clone(&factory);
            let user = *user;
            handles.push(tokio::spawn(async move {
                factory.get_for(user, "https://example.test", "token").await
            }));
        }

        let mut by_user: std::collections::HashMap<String, Vec<Arc<Database>>> =
            std::collections::HashMap::new();
        for (i, handle) in handles.into_iter().enumerate() {
            let db = handle
                .await
                .expect("task panicked")
                .expect("get_for failed");
            by_user
                .entry(users[i % users.len()].to_string())
                .or_default()
                .push(db);
        }

        // Each user opened at most once despite 10 concurrent requests each.
        assert!(
            opens.load(Ordering::SeqCst) <= users.len(),
            "opens ({}) exceeded user count ({})",
            opens.load(Ordering::SeqCst),
            users.len()
        );
        assert_eq!(opens.load(Ordering::SeqCst), users.len());

        // Every concurrent caller for the same user got the SAME handle.
        for dbs in by_user.values() {
            assert!(dbs.len() >= 2, "expected concurrent callers per user");
            for db in &dbs[1..] {
                assert!(Arc::ptr_eq(&dbs[0], db));
            }
        }

        let stats = factory.stats();
        assert_eq!(stats.misses, users.len() as u64);
        assert_eq!(stats.hits, (50 - users.len()) as u64);
        assert_eq!(stats.total_connections, users.len());
        assert!((stats.hit_rate() - 0.9).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_token_rotation_reopens_connection() {
        let opens = Arc::new(AtomicUsize::new(0));
        let factory = CachedConnectionFactory::with_connector(
            test_config(),
            String::new(),
            counting_connector(Arc::clone(&opens)),
        );

        factory
            .get_for("user-1", "https://example.test", "token-a")
            .await
            .unwrap();
        // Same credentials: cache hit, no reopen.
        factory
            .get_for("user-1", "https://example.test", "token-a")
            .await
            .unwrap();
        assert_eq!(opens.load(Ordering::SeqCst), 1);

        // Rotated token: must reopen and replace the entry.
        let rotated = factory
            .get_for("user-1", "https://example.test", "token-b")
            .await
            .unwrap();
        assert_eq!(opens.load(Ordering::SeqCst), 2);

        // New token now hits the cache.
        let again = factory
            .get_for("user-1", "https://example.test", "token-b")
            .await
            .unwrap();
        assert!(Arc::ptr_eq(&rotated, &again));
        assert_eq!(opens.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_ttl_expiry_reopens_connection() {
        let opens = Arc::new(AtomicUsize::new(0));
        let config = CachedFactoryConfig {
            ttl: Duration::from_millis(50),
            ..test_config()
        };
        let factory = CachedConnectionFactory::with_connector(
            config,
            String::new(),
            counting_connector(Arc::clone(&opens)),
        );

        factory
            .get_for("user-1", "https://example.test", "token")
            .await
            .unwrap();
        factory
            .get_for("user-1", "https://example.test", "token")
            .await
            .unwrap();
        assert_eq!(opens.load(Ordering::SeqCst), 1);

        tokio::time::sleep(Duration::from_millis(60)).await;

        factory
            .get_for("user-1", "https://example.test", "token")
            .await
            .unwrap();
        assert_eq!(opens.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_lru_eviction_bounds_cache() {
        let opens = Arc::new(AtomicUsize::new(0));
        let config = CachedFactoryConfig {
            max_connections: 1,
            ..test_config()
        };
        let factory = CachedConnectionFactory::with_connector(
            config,
            String::new(),
            counting_connector(Arc::clone(&opens)),
        );

        let first = factory
            .get_for("user-1", "https://example.test", "token")
            .await
            .unwrap();
        factory
            .get_for("user-2", "https://example.test", "token")
            .await
            .unwrap();

        assert_eq!(factory.stats().total_connections, 1);

        // user-1 was evicted: re-fetching reopens (and evicts user-2 in turn).
        let reopened = factory
            .get_for("user-1", "https://example.test", "token")
            .await
            .unwrap();
        assert_eq!(opens.load(Ordering::SeqCst), 3);
        assert!(!Arc::ptr_eq(&first, &reopened));
    }

    #[tokio::test]
    async fn test_invalidate_forces_reopen() {
        let opens = Arc::new(AtomicUsize::new(0));
        let factory = CachedConnectionFactory::with_connector(
            test_config(),
            String::new(),
            counting_connector(Arc::clone(&opens)),
        );

        let before = factory
            .get_for("user-1", "https://example.test", "token")
            .await
            .unwrap();
        factory.invalidate("user-1");
        assert_eq!(factory.stats().total_connections, 0);

        let after = factory
            .get_for("user-1", "https://example.test", "token")
            .await
            .unwrap();
        assert_eq!(opens.load(Ordering::SeqCst), 2);
        assert!(!Arc::ptr_eq(&before, &after));
    }
}
