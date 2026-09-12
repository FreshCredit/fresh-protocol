//! Shared Hrana idle-stream reconnect wrapper.
//!
//! Turso drops idle server-side Hrana streams; long-lived `libsql::Connection`s
//! then fail with 404 "stream not found" (or "stream closed") until a fresh
//! `Connection` — and therefore stream — is opened. [`ReconnectingConnection`]
//! centralises the detect -> rebuild -> retry-once handling that previously
//! existed independently in the staging, local, and cloud libSQL clients.

use std::future::Future;
use std::sync::Arc;

use tokio::sync::{RwLock, RwLockReadGuard};
use tracing::{info, warn};

/// Returns true when `err` indicates the server-side Hrana stream is gone and
/// a fresh connection (new stream) is required before retrying.
///
/// Matches the union of the error classes the staging, local, and cloud
/// clients historically treated as reconnectable: the literal Turso 404
/// "stream not found", "stream closed", and any other Hrana-protocol error.
#[must_use]
pub fn is_hrana_stream_error(err: &libsql::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("stream not found") || msg.contains("stream closed") || msg.contains("hrana")
}

/// A `libsql::Connection` that transparently rebuilds itself and retries once
/// when the server-side Hrana stream expires.
///
/// The parent `libsql::Database` handle is kept so a fresh connection (and
/// therefore a new Hrana stream) can be opened on demand. When constructed
/// without a parent database (e.g. file-backed local `SQLite` clients),
/// stream errors are returned unchanged.
#[derive(Debug)]
pub struct ReconnectingConnection {
    /// Parent database used to recreate the connection when the Hrana stream
    /// is lost. `None` for clients where reconnection is not meaningful
    /// (local `SQLite` connections do not suffer from remote stream timeouts).
    database: Option<Arc<libsql::Database>>,
    /// Current connection. Guards are only used to clone a `Connection` out
    /// (never held across await points inside this wrapper); external callers
    /// such as the staging schema initialisers may hold read guards across
    /// awaits, which is why this is an async (`tokio`) lock.
    connection: RwLock<libsql::Connection>,
}

impl ReconnectingConnection {
    /// Wrap an existing connection, optionally keeping the parent database for
    /// reconnects.
    #[must_use]
    pub fn new(database: Option<Arc<libsql::Database>>, connection: libsql::Connection) -> Self {
        Self {
            database,
            connection: RwLock::new(connection),
        }
    }

    /// Connect to `database`, keeping the handle for later reconnects.
    ///
    /// # Errors
    ///
    /// Returns an error if the initial connection cannot be opened.
    pub fn connect(database: Arc<libsql::Database>) -> libsql::Result<Self> {
        let connection = database.connect()?;
        Ok(Self::new(Some(database), connection))
    }

    /// True when a parent database is available and stream errors can be
    /// recovered by reconnecting.
    #[must_use]
    pub const fn reconnectable(&self) -> bool {
        self.database.is_some()
    }

    /// The parent database handle, when this connection can reconnect.
    #[must_use]
    pub const fn database(&self) -> Option<&Arc<libsql::Database>> {
        self.database.as_ref()
    }

    /// Read access to the underlying connection.
    ///
    /// Callers may hold the returned guard across await points, but should
    /// prefer [`Self::query`]/[`Self::execute`]/[`Self::query_one`]/
    /// [`Self::query_all`] so Hrana stream errors are covered by the
    /// reconnect-and-retry-once handling.
    pub async fn read(&self) -> RwLockReadGuard<'_, libsql::Connection> {
        self.connection.read().await
    }

    async fn current_connection(&self) -> libsql::Connection {
        self.read().await.clone()
    }

    /// Access to a usable connection.
    ///
    /// For database-backed connections (remote Turso), returns a fresh
    /// connection on every call and updates the cached connection to match, so
    /// callers never hold a stale Hrana stream. For connections without a
    /// parent database, returns the single cached connection.
    ///
    /// This is a synchronous best-effort accessor: the cache refresh is
    /// skipped when the async lock is momentarily contended.
    #[must_use]
    pub fn connection(&self) -> libsql::Connection {
        if let Some(db) = self.database.as_ref() {
            if let Ok(fresh) = db.connect() {
                if let Ok(mut guard) = self.connection.try_write() {
                    *guard = fresh.clone();
                }
                return fresh;
            }
        }
        // Without a parent database no writer can ever hold the lock
        // (reconnecting is impossible), so the cached connection is always
        // immediately available.
        loop {
            if let Ok(guard) = self.connection.try_read() {
                break guard.clone();
            }
            std::hint::spin_loop();
        }
    }

    /// Drop the current connection and open a fresh one (new Hrana stream).
    ///
    /// # Errors
    ///
    /// Returns an error when there is no parent database to reconnect through
    /// or the fresh connection cannot be opened.
    pub async fn reconnect(&self) -> libsql::Result<()> {
        let db = self.database.as_ref().ok_or_else(|| {
            libsql::Error::ConnectionFailed(
                "reconnect requested but no parent database is available".to_string(),
            )
        })?;
        let fresh = db.connect()?;
        *self.connection.write().await = fresh;
        info!("libSQL client reconnected after expired Hrana stream");
        Ok(())
    }

    /// Run `operation` against the current connection, transparently
    /// reconnecting once and retrying when the server-side Hrana stream has
    /// expired.
    ///
    /// `operation` receives a clone of the current connection on each attempt
    /// (the retried attempt uses the rebuilt connection, not the stale one).
    ///
    /// # Errors
    ///
    /// Returns the original error when it is not a stream error, the reconnect
    /// error (wrapped as `ConnectionFailed`) when rebuilding fails, or the
    /// result of the retried attempt.
    pub async fn run_with_reconnect<T, F, Fut>(&self, mut operation: F) -> libsql::Result<T>
    where
        F: FnMut(libsql::Connection) -> Fut,
        Fut: Future<Output = libsql::Result<T>>,
    {
        match operation(self.current_connection().await).await {
            Err(e) if self.reconnectable() && is_hrana_stream_error(&e) => {
                warn!("Hrana stream lost; reconnecting before retry: {e}");
                self.reconnect().await.map_err(|reconnect_err| {
                    libsql::Error::ConnectionFailed(format!(
                        "stream expired and reconnect failed: {reconnect_err}"
                    ))
                })?;
                operation(self.current_connection().await).await
            }
            other => other,
        }
    }

    /// Drop-in replacement for `libsql::Connection::query` that transparently
    /// reconnects once and retries when the server-side Hrana stream expired.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails and the reconnect-retry did not
    /// succeed.
    pub async fn query(
        &self,
        sql: &str,
        params: impl libsql::params::IntoParams,
    ) -> libsql::Result<libsql::Rows> {
        let params = params.into_params()?;
        self.run_with_reconnect(|conn| {
            let params = params.clone();
            async move { conn.query(sql, params).await }
        })
        .await
    }

    /// Drop-in replacement for `libsql::Connection::execute` that transparently
    /// reconnects once and retries when the server-side Hrana stream expired.
    ///
    /// # Errors
    ///
    /// Returns an error if the statement fails and the reconnect-retry did not
    /// succeed.
    pub async fn execute(
        &self,
        sql: &str,
        params: impl libsql::params::IntoParams,
    ) -> libsql::Result<u64> {
        let params = params.into_params()?;
        self.run_with_reconnect(|conn| {
            let params = params.clone();
            async move { conn.execute(sql, params).await }
        })
        .await
    }

    /// Execute a query and return the first row, if any.
    ///
    /// Unlike [`Self::query`], this eagerly consumes the matching row within
    /// the same attempt, so transient Hrana stream errors during row iteration
    /// are also covered by the reconnect. Returns `Ok(None)` when no row
    /// matches.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails and the reconnect-retry did not
    /// succeed.
    pub async fn query_one(
        &self,
        sql: &str,
        params: impl libsql::params::IntoParams,
    ) -> libsql::Result<Option<libsql::Row>> {
        let params = params.into_params()?;
        self.run_with_reconnect(|conn| {
            let params = params.clone();
            async move {
                let mut rows = conn.query(sql, params).await?;
                rows.next().await
            }
        })
        .await
    }

    /// Execute a query and eagerly collect all rows.
    ///
    /// Row iteration happens within the same attempt, so transient Hrana
    /// stream errors during iteration are retried once after a reconnect.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails and the reconnect-retry did not
    /// succeed.
    pub async fn query_all(
        &self,
        sql: &str,
        params: impl libsql::params::IntoParams,
    ) -> libsql::Result<Vec<libsql::Row>> {
        let params = params.into_params()?;
        self.run_with_reconnect(|conn| {
            let params = params.clone();
            async move {
                let mut rows = conn.query(sql, params).await?;
                let mut out = Vec::new();
                while let Some(row) = rows.next().await? {
                    out.push(row);
                }
                Ok(out)
            }
        })
        .await
    }

    /// Drop-in replacement for `libsql::Connection::last_insert_rowid`.
    pub async fn last_insert_rowid(&self) -> i64 {
        self.read().await.last_insert_rowid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    async fn in_memory() -> ReconnectingConnection {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .expect("in-memory database builds");
        ReconnectingConnection::connect(Arc::new(db)).expect("connects")
    }

    #[test]
    fn detects_hrana_stream_errors() {
        for msg in [
            "Hrana: `404 Not Found`: stream not found",
            "stream closed by server",
            "hrana websocket error: io error",
        ] {
            let err = libsql::Error::ConnectionFailed(msg.to_string());
            assert!(is_hrana_stream_error(&err), "expected match: {msg}");
        }
    }

    #[test]
    fn ignores_unrelated_errors() {
        for msg in [
            "no such table: users",
            "UNIQUE constraint failed",
            "syntax error",
        ] {
            let err = libsql::Error::ConnectionFailed(msg.to_string());
            assert!(!is_hrana_stream_error(&err), "unexpected match: {msg}");
        }
    }

    #[tokio::test]
    async fn query_and_execute_round_trip() {
        let conn = in_memory().await;
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)", ())
            .await
            .expect("create table");
        conn.execute(
            "INSERT INTO t (v) VALUES (?)",
            vec![libsql::Value::from("hello")],
        )
        .await
        .expect("insert");
        assert_eq!(conn.last_insert_rowid().await, 1);

        let row = conn
            .query_one(
                "SELECT v FROM t WHERE id = ?",
                vec![libsql::Value::from(1i64)],
            )
            .await
            .expect("query_one")
            .expect("row exists");
        let v: String = row.get(0).expect("text column");
        assert_eq!(v, "hello");

        let rows = conn
            .query_all("SELECT id, v FROM t", ())
            .await
            .expect("query_all");
        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn reconnect_rebuilds_connection() {
        // A file-backed database is required here: each connection to an
        // in-memory database gets its own fresh, empty database, so only a
        // shared file proves the rebuilt connection talks to the same data.
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let path = format!(
            "{}/fc_reconnect_test_{}_{}.db",
            std::env::temp_dir().display(),
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        );
        let db = libsql::Builder::new_local(&path)
            .build()
            .await
            .expect("file database builds");
        let conn = ReconnectingConnection::connect(Arc::new(db)).expect("connects");

        conn.execute("CREATE TABLE t (v TEXT)", ())
            .await
            .expect("create table");
        conn.reconnect().await.expect("manual reconnect");
        // The rebuilt connection talks to the same database.
        conn.execute(
            "INSERT INTO t (v) VALUES (?)",
            vec![libsql::Value::from("after-reconnect")],
        )
        .await
        .expect("insert after reconnect");
        let row = conn
            .query_one("SELECT v FROM t", ())
            .await
            .expect("query after reconnect")
            .expect("row exists");
        let v: String = row.get(0).expect("text column");
        assert_eq!(v, "after-reconnect");
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn retries_once_after_stream_error_then_succeeds() {
        let conn = in_memory().await;
        let attempts = AtomicUsize::new(0);
        let result = conn
            .run_with_reconnect(|conn| {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        // Simulated idle-stream expiry on the first attempt.
                        Err(libsql::Error::ConnectionFailed(
                            "Hrana: stream not found".to_string(),
                        ))
                    } else {
                        conn.query("SELECT 1", ()).await.map(|_| ())
                    }
                }
            })
            .await;
        result.expect("retry after reconnect succeeds");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn non_stream_errors_are_not_retried() {
        let conn = in_memory().await;
        let attempts = AtomicUsize::new(0);
        let result = conn
            .run_with_reconnect(|_conn| {
                attempts.fetch_add(1, Ordering::SeqCst);
                async {
                    Err::<(), libsql::Error>(libsql::Error::ConnectionFailed(
                        "no such table: missing".to_string(),
                    ))
                }
            })
            .await;
        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn unrecoverable_stream_error_without_database() {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .expect("in-memory database builds");
        let connection = db.connect().expect("connects");
        // File/memory-backed clients carry no parent database: stream errors
        // must surface unchanged instead of attempting a reconnect.
        let conn = ReconnectingConnection::new(None, connection);
        assert!(!conn.reconnectable());
        let attempts = AtomicUsize::new(0);
        let result = conn
            .run_with_reconnect(|_conn| {
                attempts.fetch_add(1, Ordering::SeqCst);
                async {
                    Err::<(), libsql::Error>(libsql::Error::ConnectionFailed(
                        "stream not found".to_string(),
                    ))
                }
            })
            .await;
        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
