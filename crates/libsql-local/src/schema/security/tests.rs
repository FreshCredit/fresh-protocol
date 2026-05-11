use libsql::{
    Builder,
    Connection,
    Value,
};

use super::initialize_security_tables;

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
