use anyhow::Result;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

// TAG: surface=database owner=platform-team rule=DB-001
/// Local `LibSQL` database client
///
/// Wraps a `libsql::Connection` and, when constructed from a shared database,
/// keeps a handle to the parent `libsql::Database` so it can reconnect after
/// transient Hrana stream errors (e.g. idle timeout on remote Turso).
#[derive(Debug)]
pub struct LocalClient {
    connection: Mutex<libsql::Connection>,
    /// Parent database used to recreate a connection if the Hrana stream is lost.
    /// `None` for clients created from a file path, where reconnection is not
    /// meaningful (local `SQLite` connections do not suffer from remote stream
    /// timeouts).
    database: Option<Arc<libsql::Database>>,
}

impl LocalClient {
    /// Create a new local client
    #[must_use = "this returns a Result that should be handled"]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn new(database_path: &str) -> Result<Self> {
        info!("Creating local LibSQL client at: {}", database_path);

        let db = libsql::Builder::new_local(database_path).build().await?;
        let connection = db.connect()?;

        Ok(Self {
            connection: Mutex::new(connection),
            database: None,
        })
    }

    /// Create a client wrapping an existing shared `libsql` database
    ///
    /// Used to place agent state (bindings, memories, `AgentFS`, controls) on
    /// the shared platform database — remote Turso in production, local file
    /// in development — instead of a separate per-instance local file.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn from_database(db: Arc<libsql::Database>) -> Result<Self> {
        let connection = db.connect()?;
        Ok(Self {
            connection: Mutex::new(connection),
            database: Some(db),
        })
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Create a new in-memory client for testing
    ///
    /// This is useful for unit tests that don't need persistent storage.
    /// Available in test builds and when `test-utils` feature is enabled.
    #[cfg(any(test, feature = "test-utils"))]
    #[must_use = "this returns a Result that should be handled"]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn new_in_memory() -> Result<Self> {
        let db = libsql::Builder::new_local(":memory:").build().await?;
        let connection = db.connect()?;
        Ok(Self {
            connection: Mutex::new(connection),
            database: None,
        })
    }

    /// Get access to the underlying connection for direct queries.
    ///
    /// For clients backed by a shared database (remote Turso), returns a fresh
    /// connection on every call so callers never hold a stale Hrana stream. The
    /// cached connection is updated to match. For file-backed clients, returns the
    /// single cached connection.
    #[must_use]
    pub fn connection(&self) -> libsql::Connection {
        if let Some(db) = self.database.as_ref() {
            if let Ok(fresh) = db.connect() {
                {
                    let mut guard = self
                        .connection
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    *guard = fresh.clone();
                }
                return fresh;
            }
        }
        self.connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Initialize database schema using modular schema definitions
    ///
    /// This function delegates to the schema module for table creation.
    /// All table definitions are in `schema/` submodules for maintainability.
    ///
    /// `HARDCODED_SCHEMA`: 150 tables total across all modules (verified 2026-08-25)
    /// See `schema/mod.rs` for the complete table inventory.
    #[must_use = "this returns a Result that should be handled"]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing local database schema with modular schema definitions");

        let conn = self.connection();
        // Delegate to the modular schema initialization
        crate::schema::initialize_all_schema_tables(&conn).await?;

        // HARDCODED_SCHEMA: 150 tables in Rust modular schema (verified 2026-08-25)
        info!("Unified database schema initialization completed (150 tables)");
        Ok(())
    }

    /// Execute a raw SQL query and return rows.
    ///
    /// Retries once on Hrana stream errors when backed by a shared database.
    /// NOTE: the returned `Rows` may hold a remote Hrana stream; callers that
    /// iterate rows across await points should prefer [`Self::query_one`] or
    /// [`Self::query_all`] so row iteration is also covered by the reconnect.
    #[must_use = "this returns a Result that should be handled"]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn query(&self, sql: &str, params: Vec<libsql::Value>) -> Result<libsql::Rows> {
        let conn = self.connection();
        match conn.query(sql, params.clone()).await {
            Ok(rows) => Ok(rows),
            Err(e) if Self::is_reconnectable(&e) && self.database.is_some() => {
                warn!(
                    "Hrana stream lost on query; reconnecting LocalClient: {}",
                    e
                );
                let new_conn = self.reconnect()?;
                new_conn
                    .query(sql, params)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))
            }
            Err(e) => Err(anyhow::anyhow!("{e}")),
        }
    }

    /// Execute a raw SQL query and return the first row, if any.
    ///
    /// Unlike [`Self::query`], this eagerly consumes the matching row within the
    /// same connection scope, so transient Hrana stream errors during row
    /// iteration are also retried. Returns `Ok(None)` when no row matches.
    #[must_use = "this returns a Result that should be handled"]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn query_one(
        &self,
        sql: &str,
        params: Vec<libsql::Value>,
    ) -> Result<Option<libsql::Row>> {
        let conn = self.connection();
        match conn.query(sql, params.clone()).await {
            Ok(mut rows) => {
                match rows.next().await {
                    Ok(row) => Ok(row),
                    Err(e) if Self::is_reconnectable(&e) && self.database.is_some() => {
                        warn!("Hrana stream lost on query_one row fetch; reconnecting LocalClient: {}", e);
                        let new_conn = self.reconnect()?;
                        let mut rows = new_conn
                            .query(sql, params)
                            .await
                            .map_err(|e| anyhow::anyhow!("{e}"))?;
                        rows.next().await.map_err(|e| anyhow::anyhow!("{e}"))
                    }
                    Err(e) => Err(anyhow::anyhow!("{e}")),
                }
            }
            Err(e) if Self::is_reconnectable(&e) && self.database.is_some() => {
                warn!(
                    "Hrana stream lost on query_one; reconnecting LocalClient: {}",
                    e
                );
                let new_conn = self.reconnect()?;
                let mut rows = new_conn
                    .query(sql, params)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                rows.next().await.map_err(|e| anyhow::anyhow!("{e}"))
            }
            Err(e) => Err(anyhow::anyhow!("{e}")),
        }
    }

    /// Execute a raw SQL query and return all rows.
    ///
    /// Eagerly collects rows within the same connection scope so transient Hrana
    /// stream errors during iteration are retried once.
    #[must_use = "this returns a Result that should be handled"]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn query_all(
        &self,
        sql: &str,
        params: Vec<libsql::Value>,
    ) -> Result<Vec<libsql::Row>> {
        let conn = self.connection();
        match Self::collect_rows(&conn, sql, &params).await {
            Ok(rows) => Ok(rows),
            Err(e) if Self::is_reconnectable(&e) && self.database.is_some() => {
                warn!(
                    "Hrana stream lost on query_all; reconnecting LocalClient: {}",
                    e
                );
                let new_conn = self.reconnect()?;
                Self::collect_rows(&new_conn, sql, &params)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))
            }
            Err(e) => Err(anyhow::anyhow!("{e}")),
        }
    }

    /// Collect all rows from a query into a vector.
    async fn collect_rows(
        conn: &libsql::Connection,
        sql: &str,
        params: &[libsql::Value],
    ) -> std::result::Result<Vec<libsql::Row>, libsql::Error> {
        let mut rows = conn.query(sql, params.to_vec()).await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(row);
        }
        Ok(out)
    }

    /// Execute a raw SQL statement and return affected rows count.
    ///
    /// Retries once on Hrana stream errors when backed by a shared database.
    #[must_use = "this returns a Result that should be handled"]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn execute(&self, sql: &str, params: Vec<libsql::Value>) -> Result<u64> {
        let conn = self.connection();
        match conn.execute(sql, params.clone()).await {
            Ok(rows) => Ok(rows),
            Err(e) if Self::is_reconnectable(&e) && self.database.is_some() => {
                warn!(
                    "Hrana stream lost on execute; reconnecting LocalClient: {}",
                    e
                );
                let new_conn = self.reconnect()?;
                new_conn
                    .execute(sql, params)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))
            }
            Err(e) => Err(anyhow::anyhow!("{e}")),
        }
    }

    /// Recreate the underlying connection from the parent database.
    ///
    /// Updates the stored connection so future operations use the new stream.
    fn reconnect(&self) -> Result<libsql::Connection> {
        let db = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No parent database available to reconnect"))?;
        let new_conn = db.connect()?;
        {
            let mut guard = self
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *guard = new_conn.clone();
        }
        Ok(new_conn)
    }

    /// Detect transient Hrana stream errors that are safe to retry after a
    /// reconnect.
    fn is_reconnectable(e: &libsql::Error) -> bool {
        let msg = e.to_string().to_lowercase();
        msg.contains("stream not found") || msg.contains("stream closed") || msg.contains("hrana")
    }
}
