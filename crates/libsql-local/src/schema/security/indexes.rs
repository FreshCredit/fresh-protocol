//! Security monitoring indexes

use anyhow::Result;
use libsql::Connection;

/// Initialize security monitoring indexes
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_security_indexes(conn: &Connection) -> Result<()> {
    // Sessions table indexes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ip_blocks_ip_address ON ip_blocks(ip_address)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_ip_blocks_expires_at ON ip_blocks(expires_at)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_rate_limit_events_ip ON rate_limit_events(ip_address, created_at)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_step_up_auth_user ON step_up_auth_requests(user_id, status)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_user_devices_user ON user_devices(user_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_user_devices_fingerprint ON user_devices(device_fingerprint)",
        (),
    )
    .await?;

    Ok(())
}
