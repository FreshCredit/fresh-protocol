// TAG: surface=database owner=platform-team rule=DB-001 test-coverage=unit
use anyhow::Result;
use libsql::Connection;

/// Add a column to a table only if it does not already exist.
///
/// This avoids the SQLite "duplicate column name" error that breaks Hrana
/// streams on Turso cloud, which in turn aborts in-flight requests such as
/// the AI assistant chat loop.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn add_column_if_not_exists(
    conn: &Connection,
    table: &str,
    column: &str,
    def: &str,
) -> Result<()> {
    let mut rows = conn
        .query(
            "SELECT 1 FROM pragma_table_info(?) WHERE name = ?",
            libsql::params![table, column],
        )
        .await?;

    if rows.next().await?.is_none() {
        let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {def}");
        conn.execute(&sql, ()).await?;
        tracing::info!("Added column {table}.{column}");
    } else {
        tracing::debug!("Column {table}.{column} already exists");
    }

    Ok(())
}

/// Helper to try creating an index, ignoring "no such column" errors
/// This is needed because embedded replicas may sync from Turso cloud
/// which could have an older schema without certain columns.
/// # Errors
///
/// Returns an error if the operation fails.
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
