use anyhow::Result;
use libsql::Connection;

use super::helpers::{
    create_arc_tables, create_bridge_tables, create_crypto_tables, create_fiat_tables,
    create_gateway_tables,
};
use tracing::info;

/// Initialize all payment tables.
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_payment_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing payments tables");

    create_fiat_tables(conn).await?;
    create_crypto_tables(conn).await?;
    create_arc_tables(conn).await?;
    create_bridge_tables(conn).await?;
    create_gateway_tables(conn).await?;

    Ok(())
}
