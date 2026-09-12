//! Security monitoring tables

use anyhow::Result;
use libsql::Connection;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize security monitoring tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_security_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing security tables");
    // Sessions table for server-side session storage
    // Used in conjunction with JWT tokens for session activity tracking
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            last_activity INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            data TEXT NOT NULL,
            ip_address TEXT,
            user_agent TEXT,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;

    // IP blocks table for brute force protection
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS ip_blocks (
            id TEXT PRIMARY KEY,
            ip_address TEXT NOT NULL,
            reason TEXT NOT NULL,
            blocked_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            expires_at DATETIME NOT NULL,
            fail_count INTEGER DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // Rate limit events table
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS rate_limit_events (
            id TEXT PRIMARY KEY,
            ip_address TEXT NOT NULL,
            request_count INTEGER NOT NULL,
            window_minutes INTEGER NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .await?;

    // Step-up authentication requests for sensitive actions from new devices
    freshcredit_libsql_common::schema::ensure_table_ddl(conn, "CREATE TABLE IF NOT EXISTS step_up_auth_requests (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            audit_event_id TEXT,
            reason TEXT NOT NULL,
            device_fingerprint TEXT,
            ip_address TEXT,
            status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'verified', 'expired', 'denied')),
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            expires_at DATETIME NOT NULL,
            verified_at DATETIME,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (audit_event_id) REFERENCES audit_events (id) ON DELETE SET NULL
        )").await?;

    // User devices table for device fingerprinting
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS user_devices (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            device_fingerprint TEXT NOT NULL,
            device_name TEXT,
            device_type TEXT,
            user_agent TEXT,
            ip_address TEXT,
            verified INTEGER NOT NULL DEFAULT 0,
            first_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            last_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            UNIQUE(user_id, device_fingerprint)
        )",
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // Compliance digests table for weekly reports
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS compliance_digests (
            id TEXT PRIMARY KEY,
            generated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            period_start DATETIME NOT NULL,
            period_end DATETIME NOT NULL,
            compliance_score REAL NOT NULL,
            critical_findings INTEGER DEFAULT 0,
            high_findings INTEGER DEFAULT 0,
            medium_findings INTEGER DEFAULT 0,
            low_findings INTEGER DEFAULT 0,
            total_findings INTEGER DEFAULT 0,
            resolved_this_period INTEGER DEFAULT 0,
            new_this_period INTEGER DEFAULT 0,
            api_keys_expiring INTEGER DEFAULT 0,
            digest_html TEXT,
            sent_at DATETIME
        )",
    )
    .await?;

    Ok(())
}
