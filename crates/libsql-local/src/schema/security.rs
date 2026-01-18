//! Security monitoring schema definitions
//!
//! Contains security tables:
//! - sessions: Server-side session storage for authenticated users
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
    // Sessions table for server-side session storage
    // Used in conjunction with JWT tokens for session activity tracking
    conn.execute(
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
        (),
    )
    .await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use libsql::{Builder, Value};

    async fn create_test_connection() -> Connection {
        let db = Builder::new_local(":memory:").build().await.unwrap();
        db.connect().unwrap()
    }

    /// Create base tables that security tables have foreign key constraints on
    async fn create_base_tables(conn: &Connection) {
        // user_profile table (referenced by user_devices, step_up_auth_requests)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS user_profile (
                id TEXT PRIMARY KEY,
                email TEXT NOT NULL
            )",
            (),
        )
        .await
        .unwrap();

        // audit_events table (referenced by step_up_auth_requests)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS audit_events (
                id TEXT PRIMARY KEY,
                event_type TEXT NOT NULL
            )",
            (),
        )
        .await
        .unwrap();
    }

    /// Create a test user for foreign key constraints
    async fn create_test_user(conn: &Connection, user_id: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO user_profile (id, email) VALUES (?1, ?2)",
            vec![
                Value::Text(user_id.into()),
                Value::Text(format!("{user_id}@example.com")),
            ],
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_initialize_security_tables_creates_all_tables() {
        let conn = create_test_connection().await;
        initialize_security_tables(&conn).await.unwrap();

        // Verify ip_blocks table exists
        let mut result = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='ip_blocks'",
                (),
            )
            .await
            .unwrap();
        let row = result.next().await.unwrap();
        assert!(row.is_some(), "ip_blocks table should exist");

        // Verify rate_limit_events table exists
        let mut result = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='rate_limit_events'",
                (),
            )
            .await
            .unwrap();
        let row = result.next().await.unwrap();
        assert!(row.is_some(), "rate_limit_events table should exist");

        // Verify step_up_auth_requests table exists
        let mut result = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='step_up_auth_requests'",
                (),
            )
            .await
            .unwrap();
        let row = result.next().await.unwrap();
        assert!(row.is_some(), "step_up_auth_requests table should exist");

        // Verify user_devices table exists
        let mut result = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='user_devices'",
                (),
            )
            .await
            .unwrap();
        let row = result.next().await.unwrap();
        assert!(row.is_some(), "user_devices table should exist");

        // Verify compliance_digests table exists
        let mut result = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='compliance_digests'",
                (),
            )
            .await
            .unwrap();
        let row = result.next().await.unwrap();
        assert!(row.is_some(), "compliance_digests table should exist");
    }

    #[tokio::test]
    async fn test_ip_blocks_crud_operations() {
        let conn = create_test_connection().await;
        initialize_security_tables(&conn).await.unwrap();

        // Insert an IP block
        conn.execute(
            "INSERT INTO ip_blocks (id, ip_address, reason, expires_at, fail_count) VALUES (?1, ?2, ?3, ?4, ?5)",
            vec![
                Value::Text("block_1".into()),
                Value::Text("192.168.1.100".into()),
                Value::Text("brute_force".into()),
                Value::Text("2024-12-31T23:59:59Z".into()),
                Value::Integer(5),
            ],
        )
        .await
        .unwrap();

        // Read the IP block
        let mut result = conn
            .query(
                "SELECT ip_address, reason, fail_count FROM ip_blocks WHERE id = ?1",
                vec![Value::Text("block_1".into())],
            )
            .await
            .unwrap();
        let row = result.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "192.168.1.100");
        assert_eq!(row.get::<String>(1).unwrap(), "brute_force");
        assert_eq!(row.get::<i64>(2).unwrap(), 5);

        // Update the IP block
        conn.execute(
            "UPDATE ip_blocks SET fail_count = ?1 WHERE id = ?2",
            vec![Value::Integer(10), Value::Text("block_1".into())],
        )
        .await
        .unwrap();

        let mut result = conn
            .query(
                "SELECT fail_count FROM ip_blocks WHERE id = ?1",
                vec![Value::Text("block_1".into())],
            )
            .await
            .unwrap();
        let row = result.next().await.unwrap().unwrap();
        assert_eq!(row.get::<i64>(0).unwrap(), 10);

        // Delete the IP block
        conn.execute(
            "DELETE FROM ip_blocks WHERE id = ?1",
            vec![Value::Text("block_1".into())],
        )
        .await
        .unwrap();

        let mut result = conn
            .query(
                "SELECT COUNT(*) FROM ip_blocks WHERE id = ?1",
                vec![Value::Text("block_1".into())],
            )
            .await
            .unwrap();
        let row = result.next().await.unwrap().unwrap();
        assert_eq!(row.get::<i64>(0).unwrap(), 0);
    }

    #[tokio::test]
    async fn test_rate_limit_events_crud_operations() {
        let conn = create_test_connection().await;
        initialize_security_tables(&conn).await.unwrap();

        // Insert a rate limit event
        conn.execute(
            "INSERT INTO rate_limit_events (id, ip_address, request_count, window_minutes) VALUES (?1, ?2, ?3, ?4)",
            vec![
                Value::Text("evt_1".into()),
                Value::Text("10.0.0.50".into()),
                Value::Integer(150),
                Value::Integer(1),
            ],
        )
        .await
        .unwrap();

        // Read the event
        let mut result = conn
            .query(
                "SELECT ip_address, request_count, window_minutes FROM rate_limit_events WHERE id = ?1",
                vec![Value::Text("evt_1".into())],
            )
            .await
            .unwrap();
        let row = result.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "10.0.0.50");
        assert_eq!(row.get::<i64>(1).unwrap(), 150);
        assert_eq!(row.get::<i64>(2).unwrap(), 1);
    }

    #[tokio::test]
    async fn test_user_devices_unique_constraint() {
        let conn = create_test_connection().await;
        // Create base tables first (user_devices has FK to user_profile)
        create_base_tables(&conn).await;
        initialize_security_tables(&conn).await.unwrap();
        // Create test user
        create_test_user(&conn, "user_123").await;

        // Insert a user device
        conn.execute(
            "INSERT INTO user_devices (id, user_id, device_fingerprint, device_name, verified) VALUES (?1, ?2, ?3, ?4, ?5)",
            vec![
                Value::Text("dev_1".into()),
                Value::Text("user_123".into()),
                Value::Text("fp_abc123".into()),
                Value::Text("Chrome on MacOS".into()),
                Value::Integer(1),
            ],
        )
        .await
        .unwrap();

        // Try to insert duplicate (same user_id + device_fingerprint)
        let result = conn
            .execute(
                "INSERT INTO user_devices (id, user_id, device_fingerprint, device_name, verified) VALUES (?1, ?2, ?3, ?4, ?5)",
                vec![
                    Value::Text("dev_2".into()),
                    Value::Text("user_123".into()),
                    Value::Text("fp_abc123".into()),
                    Value::Text("Different name".into()),
                    Value::Integer(0),
                ],
            )
            .await;

        assert!(
            result.is_err(),
            "Should not allow duplicate user_id + device_fingerprint"
        );
    }

    #[tokio::test]
    async fn test_compliance_digests_crud_operations() {
        let conn = create_test_connection().await;
        initialize_security_tables(&conn).await.unwrap();

        // Insert a compliance digest
        conn.execute(
            "INSERT INTO compliance_digests (id, period_start, period_end, compliance_score, critical_findings, high_findings, total_findings) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            vec![
                Value::Text("digest_1".into()),
                Value::Text("2024-01-01T00:00:00Z".into()),
                Value::Text("2024-01-07T23:59:59Z".into()),
                Value::Real(95.5),
                Value::Integer(0),
                Value::Integer(2),
                Value::Integer(10),
            ],
        )
        .await
        .unwrap();

        // Read the digest
        let mut result = conn
            .query(
                "SELECT compliance_score, critical_findings, total_findings FROM compliance_digests WHERE id = ?1",
                vec![Value::Text("digest_1".into())],
            )
            .await
            .unwrap();
        let row = result.next().await.unwrap().unwrap();
        assert_eq!(row.get::<f64>(0).unwrap(), 95.5);
        assert_eq!(row.get::<i64>(1).unwrap(), 0);
        assert_eq!(row.get::<i64>(2).unwrap(), 10);
    }

    #[tokio::test]
    async fn test_indexes_are_created() {
        let conn = create_test_connection().await;
        initialize_security_tables(&conn).await.unwrap();

        // Check for ip_blocks indexes
        let mut result = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_ip_blocks_ip_address'",
                (),
            )
            .await
            .unwrap();
        assert!(
            result.next().await.unwrap().is_some(),
            "idx_ip_blocks_ip_address should exist"
        );

        let mut result = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_ip_blocks_expires_at'",
                (),
            )
            .await
            .unwrap();
        assert!(
            result.next().await.unwrap().is_some(),
            "idx_ip_blocks_expires_at should exist"
        );

        // Check for user_devices index
        let mut result = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_user_devices_user'",
                (),
            )
            .await
            .unwrap();
        assert!(
            result.next().await.unwrap().is_some(),
            "idx_user_devices_user should exist"
        );
    }

    #[tokio::test]
    async fn test_step_up_auth_status_constraint() {
        let conn = create_test_connection().await;
        // Create base tables first (step_up_auth_requests has FK to user_profile)
        create_base_tables(&conn).await;
        initialize_security_tables(&conn).await.unwrap();
        // Create test user
        create_test_user(&conn, "user_1").await;

        // Valid status values
        for status in ["pending", "verified", "expired", "denied"] {
            let id = format!("step_{status}");
            let result = conn
                .execute(
                    "INSERT INTO step_up_auth_requests (id, user_id, reason, status, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    vec![
                        Value::Text(id),
                        Value::Text("user_1".into()),
                        Value::Text("new_device".into()),
                        Value::Text(status.into()),
                        Value::Text("2024-12-31T23:59:59Z".into()),
                    ],
                )
                .await;
            assert!(result.is_ok(), "Status '{status}' should be valid");
        }

        // Invalid status should fail
        let result = conn
            .execute(
                "INSERT INTO step_up_auth_requests (id, user_id, reason, status, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                vec![
                    Value::Text("step_invalid".into()),
                    Value::Text("user_1".into()),
                    Value::Text("test".into()),
                    Value::Text("invalid_status".into()),
                    Value::Text("2024-12-31T23:59:59Z".into()),
                ],
            )
            .await;
        assert!(result.is_err(), "Invalid status should fail constraint");
    }

    #[tokio::test]
    async fn test_initialize_is_idempotent() {
        let conn = create_test_connection().await;

        // Initialize twice should not fail
        initialize_security_tables(&conn).await.unwrap();
        initialize_security_tables(&conn).await.unwrap();

        // Tables should still work
        conn.execute(
            "INSERT INTO ip_blocks (id, ip_address, reason, expires_at) VALUES (?1, ?2, ?3, ?4)",
            vec![
                Value::Text("test_1".into()),
                Value::Text("1.2.3.4".into()),
                Value::Text("test".into()),
                Value::Text("2024-12-31T23:59:59Z".into()),
            ],
        )
        .await
        .unwrap();
    }
}
