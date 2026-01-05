//! Security monitoring schema definitions
//!
//! Contains security tables:
//! - ip_blocks: Blocked IP addresses for brute force protection
//! - rate_limit_events: Rate limiting event records
//! - step_up_auth_requests: Step-up authentication requests for sensitive actions
//! - user_devices: Known user devices for device fingerprinting
//! - compliance_digests: Weekly compliance digest records
//!
//! COMPLIANCE: §14 Compliance Monitoring System
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! COMPLIANCE: §5 Observability, Logging, and Audit Requirements

use anyhow::Result;
use libsql::Connection;

/// Initialize security monitoring tables
pub async fn initialize_security_tables(conn: &Connection) -> Result<()> {
    // IP blocks table for brute force protection
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ip_blocks (
            id TEXT PRIMARY KEY,
            ip_address TEXT NOT NULL,
            reason TEXT NOT NULL,
            blocked_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            expires_at DATETIME NOT NULL,
            fail_count INTEGER DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    // Rate limit events table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS rate_limit_events (
            id TEXT PRIMARY KEY,
            ip_address TEXT NOT NULL,
            request_count INTEGER NOT NULL,
            window_minutes INTEGER NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    // Step-up authentication requests for sensitive actions from new devices
    conn.execute(
        "CREATE TABLE IF NOT EXISTS step_up_auth_requests (
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
        )",
        (),
    )
    .await?;

    // User devices table for device fingerprinting
    conn.execute(
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
        (),
    )
    .await?;

    // Compliance digests table for weekly reports
    conn.execute(
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
        (),
    )
    .await?;

    // Create indexes for security tables
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_security_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}

