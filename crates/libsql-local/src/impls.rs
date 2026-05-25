use anyhow::Result;
use tracing::info;

/// Local `LibSQL` database client
#[derive(Debug)]
pub struct LocalClient {
    pub(crate) connection: libsql::Connection,
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

        Ok(Self { connection })
    }

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
        Ok(Self { connection })
    }

    /// Get access to the underlying connection for direct queries
    #[must_use]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub const fn connection(&self) -> &libsql::Connection {
        &self.connection
    }

    /// Initialize database schema using modular schema definitions
    ///
    /// This function delegates to the schema module for table creation.
    /// All table definitions are in `schema/` submodules for maintainability.
    ///
    /// `HARDCODED_SCHEMA`: 117 tables total across all modules (verified 2026-01-15)
    /// See `schema/mod.rs` for the complete table inventory.
    #[must_use = "this returns a Result that should be handled"]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing local database schema with modular schema definitions");

        // Delegate to the modular schema initialization
        crate::schema::initialize_all_schema_tables(&self.connection).await?;

        // HARDCODED_SCHEMA: 124 tables in Rust modular schema (verified 2026-01-15)
        // Added: 4 agent tables + 3 UCP tables = 7 new tables
        info!("Unified database schema initialization completed (124 tables)");
        Ok(())
    }

    /// Execute a raw SQL query and return rows
    #[must_use = "this returns a Result that should be handled"]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn query(&self, sql: &str, params: Vec<libsql::Value>) -> Result<libsql::Rows> {
        self.connection
            .query(sql, params)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// Execute a raw SQL statement and return affected rows count
    #[must_use = "this returns a Result that should be handled"]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn execute(&self, sql: &str, params: Vec<libsql::Value>) -> Result<u64> {
        self.connection
            .execute(sql, params)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}
