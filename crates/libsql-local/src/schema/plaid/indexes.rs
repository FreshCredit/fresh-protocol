//! Plaid product indexes

use anyhow::Result;
use libsql::Connection;

/// Initialize Plaid-related indexes
///
/// Currently a placeholder as Plaid tables do not have dedicated indexes
/// beyond those defined in the unified indexes module.
pub async fn initialize_plaid_indexes(_conn: &Connection) -> Result<()> {
    Ok(())
}
