#![allow(unsafe_code)]

use super::*;
use crate::{
    CircuitBreakerConfig, CircuitBreakerConnection, CircuitBreakerState, ConnectionConfig,
    ConnectionHealth, ConnectionMode, DatabaseConnection, RetryConfig,
};

// TAG: surface=database owner=platform-team rule=DB-001
static CRYPTO_PROVIDER_INIT: std::sync::Once = std::sync::Once::new();

/// Ensures a process-level rustls `CryptoProvider` is installed.
///
/// rustls is built with both the `ring` and `aws-lc-rs` features in this
/// dependency graph, so it cannot auto-select a process default and
/// `ClientConfig::builder()` panics unless one is installed first. The
/// provider is global per-process state: a lost install race is fine, any
/// installed provider satisfies rustls.
fn ensure_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return;
    }
    CRYPTO_PROVIDER_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Runs `op`, retrying a few times with backoff before panicking.
///
/// Background: the libsql remote builder loads the platform root certificate
/// store at build time, and on macOS that keychain read can transiently fail
/// under full-suite load (observed 2026-09-06: `test_create_remote_fails`
/// failed in the full workspace suite yet passes in isolation and in the
/// crate-only suite; the builder code is unchanged from `main`). A short
/// retry absorbs the transient failure without weakening the assertion — the
/// query against `http://localhost:1` below still proves no connection is
/// established by the builder itself.
async fn with_builder_retry<T, E, Fut>(mut op: impl FnMut() -> Fut) -> T
where
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut last_err: Option<E> = None;
    for attempt in 1..=3_u32 {
        match op().await {
            Ok(value) => return value,
            Err(err) => {
                last_err = Some(err);
                tokio::time::sleep(std::time::Duration::from_millis(
                    200_u64 * u64::from(attempt),
                ))
                .await;
            }
        }
    }
    panic!("libsql remote builder kept failing (transient native root store read under load?): {last_err:?}");
}

#[tokio::test]
async fn test_create_in_memory() {
    let conn = ConnectionFactory::create_in_memory().await.unwrap();

    let health = conn.health_check().await.unwrap();
    assert!(health.is_healthy);
    assert_eq!(health.mode, ConnectionMode::LocalOnly);
}

#[tokio::test]
async fn test_create_local() {
    let config = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: Some(std::path::PathBuf::from(":memory:")),
        ..Default::default()
    };

    let conn = ConnectionFactory::create_local(&config).await.unwrap();

    let health = conn.health_check().await.unwrap();
    assert!(health.is_healthy);
}

#[tokio::test]
async fn test_config_validation() {
    // Missing local path for replica mode
    let config = ConnectionConfig {
        mode: ConnectionMode::EmbeddedReplica,
        remote_url: "test".to_string(),
        local_path: None,
        ..Default::default()
    };

    // TAG: surface=database owner=platform-team rule=DB-001
    let result = ConnectionFactory::create(&config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_validate_remote_empty_url() {
    let config = ConnectionConfig {
        mode: ConnectionMode::DirectRemote,
        remote_url: String::new(),
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Remote URL"));
}

#[tokio::test]
async fn test_validate_local_missing_path() {
    let config = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: None,
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Local path"));
}

#[tokio::test]
async fn test_validate_local_ok() {
    let config = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: Some(std::path::PathBuf::from(":memory:")),
        ..Default::default()
    };
    assert!(config.validate().is_ok());
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_create_with_fallback_primary_succeeds() {
    let primary = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: Some(std::path::PathBuf::from(":memory:")),
        ..Default::default()
    };
    let fallback = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: Some(std::path::PathBuf::from(":memory:")),
        ..Default::default()
    };

    let result = ConnectionFactory::create_with_fallback(&primary, &fallback).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_create_with_fallback_both_fail() {
    let primary = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: None,
        ..Default::default()
    };
    let fallback = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: None,
        ..Default::default()
    };

    let result = ConnectionFactory::create_with_fallback(&primary, &fallback).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_create_with_circuit_breaker() {
    let config = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: Some(std::path::PathBuf::from(":memory:")),
        ..Default::default()
    };
    let cb_config = CircuitBreakerConfig::default();

    // TAG: surface=database owner=platform-team rule=DB-001
    let result = ConnectionFactory::create_with_circuit_breaker(&config, cb_config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_circuit_breaker_with_local_db() {
    use crate::connections::LocalConnection;

    let local = LocalConnection::in_memory().await.unwrap();
    let cb = CircuitBreakerConnection::new(Arc::new(local), CircuitBreakerConfig::default());

    // Should start closed
    assert_eq!(cb.state().await, CircuitBreakerState::Closed);

    // Execute a query through circuit breaker
    let mut rows = cb.query("SELECT 1 as val", vec![]).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let val: i32 = row.get(0).unwrap();
    assert_eq!(val, 1);

    // Execute statement
    let affected = cb
        .execute("CREATE TABLE cb_test (id INTEGER)", vec![])
        .await
        .unwrap();
    assert_eq!(affected, 0);

    // Health check should reflect circuit breaker state
    let health = cb.health_check().await.unwrap();
    assert!(health.is_healthy);

    // Stats should show successes
    let stats = cb.stats().await;
    assert_eq!(stats.state, CircuitBreakerState::Closed);
    assert!(stats.successes > 0);

    // Trip the circuit
    cb.trip().await;
    assert_eq!(cb.state().await, CircuitBreakerState::Open);

    // TAG: surface=database owner=platform-team rule=DB-001
    // Health check should now be unhealthy
    let health = cb.health_check().await.unwrap();
    assert!(!health.is_healthy);

    // Reset the circuit
    cb.reset().await;
    assert_eq!(cb.state().await, CircuitBreakerState::Closed);
}

#[tokio::test]
async fn test_circuit_breaker_execute_batch_and_sync() {
    use crate::connections::LocalConnection;

    let local = LocalConnection::in_memory().await.unwrap();
    let cb = CircuitBreakerConnection::new(Arc::new(local), CircuitBreakerConfig::default());

    // execute_batch
    cb.execute_batch("CREATE TABLE batch_test (id INTEGER); INSERT INTO batch_test VALUES (1);")
        .await
        .unwrap();

    // sync is a no-op for local but should work
    cb.sync().await.unwrap();
}

#[test]
fn test_connection_health_helpers() {
    let healthy = ConnectionHealth::healthy(ConnectionMode::LocalOnly, 5);
    assert!(healthy.is_healthy);
    assert_eq!(healthy.mode, ConnectionMode::LocalOnly);
    assert_eq!(healthy.latency_ms, 5);

    let unhealthy = ConnectionHealth::unhealthy(ConnectionMode::DirectRemote, 100);
    assert!(!unhealthy.is_healthy);
    assert_eq!(unhealthy.mode, ConnectionMode::DirectRemote);
    assert_eq!(unhealthy.latency_ms, 100);
}

#[test]
fn test_connection_mode_display() {
    assert_eq!(ConnectionMode::LocalOnly.to_string(), "local-only");
    assert_eq!(ConnectionMode::DirectRemote.to_string(), "direct-remote");
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_database_connection_ext() {
    use crate::DatabaseConnectionExt;

    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("test_ext_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&db_path);

    let conn = crate::connections::LocalConnection::connect(&db_path)
        .await
        .unwrap();
    conn.execute(
        "CREATE TABLE ext_test (id INTEGER PRIMARY KEY, name TEXT)",
        vec![],
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO ext_test (name) VALUES (?)",
        vec![libsql::Value::Text("alice".to_string())],
    )
    .await
    .unwrap();

    // query_optional with existing row
    let row = conn
        .query_optional("SELECT name FROM ext_test WHERE id = 1", vec![])
        .await
        .unwrap();
    assert!(row.is_some());

    // query_optional with no row
    let row = conn
        .query_optional("SELECT name FROM ext_test WHERE id = 999", vec![])
        .await
        .unwrap();
    assert!(row.is_none());

    // TAG: surface=database owner=platform-team rule=DB-001
    // query_one with existing row
    let row = conn
        .query_one("SELECT name FROM ext_test WHERE id = 1", vec![])
        .await
        .unwrap();
    let name: String = row.get(0).unwrap();
    assert_eq!(name, "alice");

    // query_one with no row should error
    let result = conn
        .query_one("SELECT name FROM ext_test WHERE id = 999", vec![])
        .await;
    assert!(result.is_err());

    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn test_with_retry_success() {
    let config = RetryConfig {
        initial_delay: std::time::Duration::from_millis(1),
        max_delay: std::time::Duration::from_millis(10),
        max_attempts: 3,
        jitter_percentage: 0.0,
    };

    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let attempts_clone = attempts.clone();
    let result = crate::with_retry(&config, move || {
        let a = attempts_clone.clone();
        async move {
            a.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok::<_, anyhow::Error>(42)
        }
    })
    .await;

    assert_eq!(result.unwrap(), 42);
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_with_retry_eventual_success() {
    let config = RetryConfig {
        initial_delay: std::time::Duration::from_millis(1),
        max_delay: std::time::Duration::from_millis(10),
        max_attempts: 5,
        jitter_percentage: 0.0,
    };

    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let attempts_clone = attempts.clone();
    let result = crate::with_retry(&config, move || {
        let a = attempts_clone.clone();
        async move {
            let count = a.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if count < 3 {
                Err(anyhow::anyhow!("fail {count}"))
            } else {
                Ok::<_, anyhow::Error>("success")
            }
        }
    })
    .await;

    assert_eq!(result.unwrap(), "success");
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_with_retry_exhausted() {
    let config = RetryConfig {
        initial_delay: std::time::Duration::from_millis(1),
        max_delay: std::time::Duration::from_millis(10),
        max_attempts: 2,
        jitter_percentage: 0.0,
    };

    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let attempts_clone = attempts.clone();
    let result = crate::with_retry(&config, move || {
        let a = attempts_clone.clone();
        async move {
            a.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err::<(), _>(anyhow::anyhow!("always fail"))
        }
    })
    .await;

    // TAG: surface=database owner=platform-team rule=DB-001
    assert!(result.is_err());
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 3); // initial + 2 retries
}

#[test]
#[serial_test::serial]
fn test_retry_config_from_env() {
    // Set env vars
    unsafe {
        std::env::set_var("DB_RETRY_INITIAL_DELAY_MS", "500");
        std::env::set_var("DB_RETRY_MAX_DELAY_MS", "5000");
        std::env::set_var("DB_RETRY_MAX_ATTEMPTS", "7");
    }

    let config = RetryConfig::from_env();
    assert_eq!(config.initial_delay, std::time::Duration::from_millis(500));
    assert_eq!(config.max_delay, std::time::Duration::from_millis(5000));
    assert_eq!(config.max_attempts, 7);

    // Clean up
    unsafe {
        std::env::remove_var("DB_RETRY_INITIAL_DELAY_MS");
        std::env::remove_var("DB_RETRY_MAX_DELAY_MS");
        std::env::remove_var("DB_RETRY_MAX_ATTEMPTS");
    }
}

#[test]
#[serial_test::serial]
fn test_circuit_breaker_config_from_env() {
    unsafe {
        std::env::set_var("DB_CB_FAILURE_THRESHOLD", "10");
        std::env::set_var("DB_CB_RECOVERY_TIMEOUT_SECS", "60");
        std::env::set_var("DB_CB_HALF_OPEN_MAX_CALLS", "5");
        std::env::set_var("DB_CB_SUCCESS_THRESHOLD", "3");
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    let config = CircuitBreakerConfig::from_env();
    assert_eq!(config.failure_threshold, 10);
    assert_eq!(config.recovery_timeout_secs, 60);
    assert_eq!(config.half_open_max_calls, 5);
    assert_eq!(config.success_threshold, 3);

    unsafe {
        std::env::remove_var("DB_CB_FAILURE_THRESHOLD");
        std::env::remove_var("DB_CB_RECOVERY_TIMEOUT_SECS");
        std::env::remove_var("DB_CB_HALF_OPEN_MAX_CALLS");
        std::env::remove_var("DB_CB_SUCCESS_THRESHOLD");
    }
}

#[test]
#[serial_test::serial]
fn test_connection_config_from_env() {
    unsafe {
        std::env::set_var("LIBSQL_MODE", "local");
        std::env::set_var("LIBSQL_LOCAL_PATH", "/tmp/test.db");
        std::env::set_var("LIBSQL_ENABLE_FALLBACK", "true");
        std::env::set_var("LIBSQL_TIMEOUT_SECS", "45");
        std::env::set_var("LIBSQL_MAX_RETRIES", "5");
    }

    let config = ConnectionConfig::from_env().unwrap();
    assert_eq!(config.mode, ConnectionMode::LocalOnly);
    assert_eq!(
        config.local_path,
        Some(std::path::PathBuf::from("/tmp/test.db"))
    );
    assert!(config.enable_fallback);
    assert_eq!(config.timeout_secs, 45);
    assert_eq!(config.max_retries, 5);

    unsafe {
        std::env::remove_var("LIBSQL_MODE");
        std::env::remove_var("LIBSQL_LOCAL_PATH");
        std::env::remove_var("LIBSQL_ENABLE_FALLBACK");
        std::env::remove_var("LIBSQL_TIMEOUT_SECS");
        std::env::remove_var("LIBSQL_MAX_RETRIES");
    }
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_create_local_with_path() {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("test_factory_local_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&db_path);

    let conn = ConnectionFactory::create_local_with_path(&db_path)
        .await
        .unwrap();
    let health = conn.health_check().await.unwrap();
    assert!(health.is_healthy);

    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
#[serial_test::serial(remote_connector)]
async fn test_remote_connection_new_and_query_fails() {
    ensure_crypto_provider();
    // Builder succeeds even with bad URL; connection is lazy
    let db = with_builder_retry(|| {
        libsql::Builder::new_remote("http://localhost:1".to_string(), "token".to_string()).build()
    })
    .await;
    let conn = crate::connections::RemoteConnection::new(db);

    // database() accessor works
    let _ = conn.database();

    // Actual query fails because the endpoint is unreachable
    let result = conn.query("SELECT 1", vec![]).await;
    assert!(result.is_err());

    // Health check should report unhealthy
    let health = conn.health_check().await.unwrap();
    assert!(!health.is_healthy);
    assert_eq!(health.mode, ConnectionMode::DirectRemote);
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
#[serial_test::serial(remote_connector)]
async fn test_create_remote_with_url_query_fails() {
    ensure_crypto_provider();
    let conn = with_builder_retry(|| {
        ConnectionFactory::create_remote_with_url("http://localhost:1", "token")
    })
    .await;
    // Query should fail
    let result = conn.query("SELECT 1", vec![]).await;
    assert!(result.is_err());
}

#[tokio::test]
#[serial_test::serial]
async fn test_from_env_local() {
    unsafe {
        std::env::set_var("LIBSQL_MODE", "local");
        std::env::set_var("LIBSQL_LOCAL_PATH", ":memory:");
    }

    let result = ConnectionFactory::from_env().await;
    assert!(result.is_ok());

    unsafe {
        std::env::remove_var("LIBSQL_MODE");
        std::env::remove_var("LIBSQL_LOCAL_PATH");
    }
}

#[tokio::test]
#[serial_test::serial]
async fn test_create_with_circuit_breaker_from_env() {
    unsafe {
        std::env::set_var("LIBSQL_MODE", "local");
        std::env::set_var("LIBSQL_LOCAL_PATH", ":memory:");
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    let result = ConnectionFactory::create_with_circuit_breaker_from_env().await;
    assert!(result.is_ok());

    unsafe {
        std::env::remove_var("LIBSQL_MODE");
        std::env::remove_var("LIBSQL_LOCAL_PATH");
    }
}

#[tokio::test]
async fn test_create_with_cb_and_fallback_primary_succeeds() {
    let primary = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: Some(std::path::PathBuf::from(":memory:")),
        ..Default::default()
    };
    let fallback = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: Some(std::path::PathBuf::from(":memory:")),
        ..Default::default()
    };
    let cb_config = CircuitBreakerConfig::default();

    let result =
        ConnectionFactory::create_with_cb_and_fallback(&primary, &fallback, cb_config).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial_test::serial(remote_connector)]
async fn test_create_remote_fails() {
    ensure_crypto_provider();
    let config = ConnectionConfig {
        mode: ConnectionMode::DirectRemote,
        remote_url: "http://localhost:1".to_string(),
        auth_token: "token".to_string(),
        ..Default::default()
    };
    // Builder succeeds lazily (no networking until first query).
    let conn = with_builder_retry(|| ConnectionFactory::create_remote(&config)).await;
    // Query fails because endpoint is unreachable
    let err = conn.query("SELECT 1", vec![]).await;
    assert!(err.is_err());
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_replica_connection_with_local_db() {
    use crate::connections::ReplicaConnection;

    // Use a temp file to avoid any in-memory connection sharing quirks with sync()
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join(format!("test_replica_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&db_path);

    let db = libsql::Builder::new_local(&db_path).build().await.unwrap();
    let conn = ReplicaConnection::new(db, Some(3600));

    assert_eq!(conn.connection_mode(), ConnectionMode::EmbeddedReplica);
    assert_eq!(conn.sync_interval_secs(), Some(3600));

    // query should work
    let mut rows = conn.query("SELECT 1 as val", vec![]).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let val: i32 = row.get(0).unwrap();
    assert_eq!(val, 1);

    // execute should work
    let affected = conn
        .execute("CREATE TABLE rep_test (id INTEGER)", vec![])
        .await
        .unwrap();
    assert_eq!(affected, 0);

    // execute_batch should work
    conn.execute_batch("INSERT INTO rep_test VALUES (1)")
        .await
        .unwrap();

    // health_check
    let health = conn.health_check().await.unwrap();
    assert!(health.is_healthy);
    assert_eq!(health.mode, ConnectionMode::EmbeddedReplica);

    // TAG: surface=database owner=platform-team rule=DB-001
    // sync on local db fails (no remote)
    let err = conn.sync().await;
    assert!(err.is_err());

    // last_sync should be None initially
    assert!(conn.last_sync().await.is_none());

    // database() accessor
    let _ = conn.database();

    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
#[serial_test::serial(remote_connector)]
async fn test_create_replica_fails() {
    ensure_crypto_provider();
    let config = ConnectionConfig {
        mode: ConnectionMode::EmbeddedReplica,
        remote_url: "http://localhost:1".to_string(),
        auth_token: "token".to_string(),
        local_path: Some(std::path::PathBuf::from(":memory:")),
        ..Default::default()
    };
    let result = ConnectionFactory::create_replica(&config).await;
    assert!(result.is_err());
}

#[tokio::test]
#[serial_test::serial(remote_connector)]
async fn test_create_replica_with_params_fails() {
    ensure_crypto_provider();
    let result = ConnectionFactory::create_replica_with_params(
        ":memory:",
        "http://localhost:1",
        "token",
        None,
    )
    .await;
    assert!(result.is_err());
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_circuit_breaker_auto_open_via_failures() {
    use crate::connections::LocalConnection;

    let local = LocalConnection::in_memory().await.unwrap();
    let cb = CircuitBreakerConnection::new(
        Arc::new(local),
        CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_secs: 30,
            half_open_max_calls: 3,
            success_threshold: 2,
        },
    );

    assert_eq!(cb.state().await, CircuitBreakerState::Closed);

    // Execute invalid SQL to trigger a failure
    let err = cb.execute("NOT_A_SQL_STATEMENT", vec![]).await;
    assert!(err.is_err());

    // Circuit should now be open
    assert_eq!(cb.state().await, CircuitBreakerState::Open);

    // Subsequent calls should be rejected immediately
    let err = cb.query("SELECT 1", vec![]).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("OPEN"));
}

#[tokio::test]
async fn test_circuit_breaker_half_open_failure_returns_to_open() {
    use crate::connections::LocalConnection;

    let local = LocalConnection::in_memory().await.unwrap();
    let cb = CircuitBreakerConnection::new(
        Arc::new(local),
        CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_secs: 0,
            half_open_max_calls: 3,
            success_threshold: 2,
        },
    );

    // TAG: surface=database owner=platform-team rule=DB-001
    // Trip and wait
    cb.trip().await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Execute bad SQL: transitions to half-open, fails, goes back to open
    let err = cb.execute("BAD_SQL", vec![]).await;
    assert!(err.is_err());
    assert_eq!(cb.state().await, CircuitBreakerState::Open);
}

#[tokio::test]
async fn test_circuit_breaker_open_rejects_with_remaining_time() {
    use crate::connections::LocalConnection;

    let local = LocalConnection::in_memory().await.unwrap();
    let cb = CircuitBreakerConnection::new(
        Arc::new(local),
        CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_secs: 60,
            half_open_max_calls: 3,
            success_threshold: 2,
        },
    );

    cb.trip().await;
    assert_eq!(cb.state().await, CircuitBreakerState::Open);

    // Query immediately should be rejected (still in timeout)
    let err = cb.query("SELECT 1", vec![]).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("OPEN"));

    // Stats should show rejected count increased
    let stats = cb.stats().await;
    assert!(stats.rejected > 0);
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_create_with_cb_and_fallback_both_fail() {
    let primary = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: None,
        ..Default::default()
    };
    let fallback = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: None,
        ..Default::default()
    };
    let cb_config = CircuitBreakerConfig::default();

    let result =
        ConnectionFactory::create_with_cb_and_fallback(&primary, &fallback, cb_config).await;
    assert!(result.is_err());
}

#[tokio::test]
#[serial_test::serial]
async fn test_circuit_breaker_from_env() {
    use crate::connections::LocalConnection;

    unsafe {
        std::env::set_var("DB_CB_FAILURE_THRESHOLD", "3");
        std::env::set_var("DB_CB_RECOVERY_TIMEOUT_SECS", "0");
    }

    let local = LocalConnection::in_memory().await.unwrap();
    let cb = CircuitBreakerConnection::from_env(Arc::new(local)).unwrap();
    assert_eq!(cb.state().await, CircuitBreakerState::Closed);

    unsafe {
        std::env::remove_var("DB_CB_FAILURE_THRESHOLD");
        std::env::remove_var("DB_CB_RECOVERY_TIMEOUT_SECS");
    }
}

#[tokio::test]
async fn test_circuit_breaker_half_open_transitions() {
    use crate::connections::LocalConnection;

    // TAG: surface=database owner=platform-team rule=DB-001
    let local = LocalConnection::in_memory().await.unwrap();
    let cb = CircuitBreakerConnection::new(
        Arc::new(local),
        CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_secs: 0,
            half_open_max_calls: 1,
            success_threshold: 1,
        },
    );

    // Initially closed
    assert_eq!(cb.state().await, CircuitBreakerState::Closed);

    // Trip manually to open
    cb.trip().await;
    assert_eq!(cb.state().await, CircuitBreakerState::Open);

    // Wait a tiny bit so recovery timeout passes
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Now a query should transition to half-open and succeed, closing the circuit
    let result = cb.query("SELECT 1", vec![]).await;
    assert!(result.is_ok());
    assert_eq!(cb.state().await, CircuitBreakerState::Closed);
}

#[tokio::test]
async fn test_circuit_breaker_half_open_limit() {
    use crate::connections::LocalConnection;

    let local = LocalConnection::in_memory().await.unwrap();
    let cb = CircuitBreakerConnection::new(
        Arc::new(local),
        CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout_secs: 0,
            half_open_max_calls: 1,
            success_threshold: 2, // Need 2 successes to close; first call transitions+executes
        },
    );

    // TAG: surface=database owner=platform-team rule=DB-001
    cb.trip().await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // First call after timeout: transitions to half-open, executes (0 < 1), stays half-open
    let result = cb.query("SELECT 1", vec![]).await;
    assert!(result.is_ok());
    assert_eq!(cb.state().await, CircuitBreakerState::HalfOpen);

    // Second call: half_open_calls is back to 0 after record_success decrement,
    // so 0 < 1 is true, it executes again and closes the circuit
    let result = cb.query("SELECT 1", vec![]).await;
    assert!(result.is_ok());
    assert_eq!(cb.state().await, CircuitBreakerState::Closed);
}

#[tokio::test]
async fn test_circuit_breaker_open_rejects() {
    use crate::connections::LocalConnection;

    let local = LocalConnection::in_memory().await.unwrap();
    let cb = CircuitBreakerConnection::new(Arc::new(local), CircuitBreakerConfig::default());

    // Trip the circuit
    cb.trip().await;
    assert_eq!(cb.state().await, CircuitBreakerState::Open);

    // Query should fail with circuit open error
    let err = cb.query("SELECT 1", vec![]).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("OPEN"));

    // execute should also fail
    let err = cb.execute("SELECT 1", vec![]).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("OPEN"));

    // execute_batch should also fail
    let err = cb.execute_batch("SELECT 1").await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("OPEN"));

    // sync should also fail
    let err = cb.sync().await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("OPEN"));
}
