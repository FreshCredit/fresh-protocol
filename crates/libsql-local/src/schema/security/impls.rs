// TAG: surface=database owner=platform-team rule=DB-001 test-coverage=unit
use anyhow::Result;
use libsql::Connection;

/// Initialize security monitoring tables and indexes
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_security_tables(conn: &Connection) -> Result<()> {
    super::tables::initialize_security_tables(conn).await?;
    super::indexes::initialize_security_indexes(conn).await?;
    Ok(())
}
