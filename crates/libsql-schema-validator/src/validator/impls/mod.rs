// TAG: surface=database owner=platform-team rule=DB-001 test-coverage=unit
use crate::types::SchemaValidator;

mod core;
mod helpers;
#[cfg(test)]
mod tests;

impl SchemaValidator {
    /// Create a new schema validator
    #[must_use]
    pub const fn new() -> Self {
        Self {
            staging_connection: None,
            local_connection: None,
            cloud_connection: None,
        }
    }

    /// Add staging database connection
    #[must_use]
    pub fn with_staging(mut self, connection: libsql::Connection) -> Self {
        self.staging_connection = Some(connection);
        self
    }

    /// Add local database connection
    #[must_use]
    pub fn with_local(mut self, connection: libsql::Connection) -> Self {
        self.local_connection = Some(connection);
        self
    }

    /// Add cloud database connection
    #[must_use]
    pub fn with_cloud(mut self, connection: libsql::Connection) -> Self {
        self.cloud_connection = Some(connection);
        self
    }
}

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}
