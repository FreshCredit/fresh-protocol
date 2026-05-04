use anyhow::Result;
use libsql::Connection;

/// Helper to try creating an index, ignoring "no such column" errors
/// This is needed because embedded replicas may sync from Turso cloud
/// which could have an older schema without certain columns.
pub async fn try_create_index(conn: &Connection, sql: &str) -> Result<()> {
    match conn.execute(sql, ()).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let err_msg = e.to_string();
            // Ignore "no such column" and "no such table" errors
            // These happen when syncing from older cloud schemas
            if err_msg.contains("no such column") || err_msg.contains("no such table") {
                tracing::debug!(
                    "Skipping index creation (column/table not in synced schema): {sql}"
                );
                Ok(())
            } else {
                Err(anyhow::anyhow!("{e}"))
            }
        }
    }
}
