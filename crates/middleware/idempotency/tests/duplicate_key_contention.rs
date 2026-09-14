//! Jepsen-style duplicate-key concurrency tests for the durable idempotency
//! store.
//!
//! These tests probe the store's public contract
//! ([`DurableIdempotencyStore`]):
//!
//! 1. **Exactly-once claim** — N concurrent claims for the same key produce
//!    exactly one [`ClaimOutcome::NewClaim`]; this is the at-most-once
//!    execution property that prevents double payments.
//! 2. **Replay safety** — after the winner completes, every duplicate claim
//!    observes [`ClaimOutcome::Completed`] with a byte-identical response.
//! 3. **Stale reclaim (crash recovery)** — an `in_flight` row whose
//!    `created_at` predates the staleness timeout is atomically reclaimable,
//!    so a crashed handler cannot wedge a key forever.
//! 4. **No key leakage** — N distinct keys claimed and completed concurrently
//!    are independently replayable, with no cross-key response mixing.
//!
//! Tests 1, 2 and 4 use a shared in-memory platform database
//! (`libsql::Builder::new_local(":memory:")`); the store holds a single
//! connection, so the in-memory database is shared by all operations. Test 3
//! needs a second direct-SQL connection to backdate `created_at`; because
//! libsql gives every connection to `:memory:` its own private database, that
//! test uses a temporary file-backed database instead.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use freshcredit_core_timing::{Clock, MockClock};
use freshcredit_middleware_idempotency::{
    ClaimOutcome, DurableIdempotencyStore, IdempotencyConfig, StoredResponse,
};
use tokio::time::timeout;

const CLAIM_TIMEOUT: Duration = Duration::from_secs(10);
const RESPONSE_TTL: Duration = Duration::from_secs(3600);
const STALE_SECONDS: u64 = 300;
const SCOPE: &str = "jepsen";
const TASKS: usize = 16;

/// `MockClock` requires `&mut` to advance; the store only needs `&dyn Clock`.
struct TestClock {
    inner: Mutex<MockClock>,
}

impl TestClock {
    fn new() -> Self {
        Self {
            inner: Mutex::new(MockClock::new(chrono::Utc::now())),
        }
    }
}

impl Clock for TestClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.inner.lock().expect("clock lock").now()
    }
}

/// Build a store over a fresh in-memory platform database.
async fn memory_store() -> Arc<DurableIdempotencyStore> {
    let db = libsql::Builder::new_local(":memory:")
        .build()
        .await
        .expect("in-memory platform db builds");
    let config = IdempotencyConfig::new(RESPONSE_TTL).with_stale_seconds(STALE_SECONDS);
    let store =
        DurableIdempotencyStore::from_database(config, Arc::new(TestClock::new()), Arc::new(db))
            .await
            .expect("durable store constructs");
    Arc::new(store)
}

/// A stuck claim fails the test instead of hanging the suite.
async fn claim_guarded(store: &DurableIdempotencyStore, key: &str) -> ClaimOutcome {
    match timeout(CLAIM_TIMEOUT, store.claim(key, SCOPE)).await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(err)) => panic!("claim({key}) failed: {err}"),
        Err(elapsed) => panic!("claim({key}) timed out after {CLAIM_TIMEOUT:?}: {elapsed}"),
    }
}

fn sample_response(clock: &TestClock, status: u16, body: Vec<u8>) -> StoredResponse {
    StoredResponse::new(
        status,
        vec![("content-type".to_string(), "application/json".to_string())],
        body,
        RESPONSE_TTL,
        clock,
    )
}

/// Scenario 1: 16 tasks claim the same key at once. Exactly one may observe
/// `NewClaim` — a second `NewClaim` would mean the keyed request could
/// execute twice (e.g. a double payment).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_claims_have_exactly_one_winner() {
    let store = memory_store().await;
    let outcomes = Arc::new(Mutex::new(Vec::new()));
    let barrier = Arc::new(tokio::sync::Barrier::new(TASKS));

    let mut handles = Vec::new();
    for _ in 0..TASKS {
        let store = Arc::clone(&store);
        let outcomes = Arc::clone(&outcomes);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            // Maximize overlap: everyone piles onto the claim together.
            barrier.wait().await;
            let outcome = claim_guarded(&store, "hot-key").await;
            outcomes.lock().expect("outcomes lock").push(outcome);
        }));
    }
    for handle in handles {
        handle.await.expect("claim task panicked");
    }

    let (new_claims, in_progress, completed, snapshot) = {
        let outcomes = outcomes.lock().expect("outcomes lock");
        let new_claims = outcomes
            .iter()
            .filter(|o| **o == ClaimOutcome::NewClaim)
            .count();
        let in_progress = outcomes
            .iter()
            .filter(|o| **o == ClaimOutcome::InProgress)
            .count();
        let completed = outcomes
            .iter()
            .filter(|o| matches!(o, ClaimOutcome::Completed(_)))
            .count();
        (new_claims, in_progress, completed, outcomes.clone())
    };
    assert_eq!(
        new_claims, 1,
        "exactly one NewClaim expected across {TASKS} concurrent claims, got {new_claims}: {snapshot:?}"
    );
    assert_eq!(
        in_progress,
        TASKS - 1,
        "all non-winners must observe InProgress: {snapshot:?}"
    );
    assert_eq!(completed, 0, "no response exists yet: {snapshot:?}");
}

/// Scenario 2 (replay safety): the winner completes with a response; then 16
/// fresh tasks claim the same key and every one must observe `Completed`
/// with a byte-identical response — the core duplicate-submission property.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn completed_key_replays_byte_identical_response() {
    let store = memory_store().await;

    // One winner executes the "request" and stores its response.
    assert_eq!(
        claim_guarded(&store, "pay-123").await,
        ClaimOutcome::NewClaim
    );
    let clock = TestClock::new();
    let stored = sample_response(&clock, 201, b"payment-settled".to_vec());
    store
        .complete("pay-123", &stored)
        .await
        .expect("complete succeeds");

    // get() must serve the same bytes.
    let fetched = store
        .get("pay-123")
        .await
        .expect("get succeeds")
        .expect("get returns the stored response");
    assert_eq!(fetched.body, stored.body);

    // 16 duplicate submissions: all must replay the identical response.
    let replays = Arc::new(Mutex::new(Vec::new()));
    let barrier = Arc::new(tokio::sync::Barrier::new(TASKS));
    let mut handles = Vec::new();
    for _ in 0..TASKS {
        let store = Arc::clone(&store);
        let replays = Arc::clone(&replays);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            let outcome = claim_guarded(&store, "pay-123").await;
            replays.lock().expect("replays lock").push(outcome);
        }));
    }
    for handle in handles {
        handle.await.expect("replay task panicked");
    }

    let replays = replays.lock().expect("replays lock");
    assert_eq!(
        replays.len(),
        TASKS,
        "every duplicate submission must get an outcome"
    );
    for outcome in replays.iter() {
        match outcome {
            ClaimOutcome::Completed(response) => {
                assert_eq!(response.status, stored.status);
                assert_eq!(response.headers, stored.headers);
                assert_eq!(
                    response.body, stored.body,
                    "replayed response must be byte-identical to the stored one"
                );
            }
            other => panic!("expected Completed replay, got {other:?}"),
        }
    }
}

/// Scenario 3 (crash recovery): a holder claims a key and dies without
/// completing. Once the row is older than the staleness timeout, a new claim
/// must succeed — but a fresh row must NOT be reclaimable (control).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_in_flight_row_is_reclaimed_but_fresh_row_is_not() {
    // File-backed database: this test needs a second direct-SQL connection to
    // backdate `created_at`, and libsql `:memory:` databases are private to
    // each connection.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "fc-idempotency-jepsen-{}-{nanos}.db",
        std::process::id()
    ));
    let db = libsql::Builder::new_local(&path)
        .build()
        .await
        .expect("platform db builds");
    let db = Arc::new(db);
    let config = IdempotencyConfig::new(RESPONSE_TTL).with_stale_seconds(60);
    let store = Arc::new(
        DurableIdempotencyStore::from_database(config, Arc::new(TestClock::new()), Arc::clone(&db))
            .await
            .expect("durable store constructs"),
    );

    // Crash: claim, then die before completing.
    assert_eq!(
        claim_guarded(&store, "crashed").await,
        ClaimOutcome::NewClaim
    );

    // Control: a fresh in-flight row is still honored.
    assert_eq!(
        claim_guarded(&store, "crashed").await,
        ClaimOutcome::InProgress
    );

    // Backdate the row past the 60s staleness timeout, as if the crash
    // happened long ago.
    let conn = db.connect().expect("second connection");
    conn.execute(
        "UPDATE idempotency_records SET created_at = unixepoch() - 3600 WHERE key = 'crashed'",
        (),
    )
    .await
    .expect("backdate update");

    // Crash recovery: the stale claim is atomically reclaimed.
    assert_eq!(
        claim_guarded(&store, "crashed").await,
        ClaimOutcome::NewClaim
    );

    // The reclaimer can complete and replay normally.
    let clock = TestClock::new();
    store
        .complete(
            "crashed",
            &sample_response(&clock, 200, b"recovered".to_vec()),
        )
        .await
        .expect("complete after reclaim");
    match claim_guarded(&store, "crashed").await {
        ClaimOutcome::Completed(response) => assert_eq!(response.body, b"recovered".to_vec()),
        other => panic!("expected Completed after reclaim+complete, got {other:?}"),
    }

    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(std::path::PathBuf::from(format!(
            "{}{suffix}",
            path.display()
        )));
    }
}

/// Scenario 4 (no key leakage): 32 tasks each claim and complete a distinct
/// key concurrently; afterwards every key must replay its own response and
/// nothing else.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn distinct_keys_claimed_concurrently_replay_independently() {
    const KEYS: usize = 32;

    let store = memory_store().await;
    let barrier = Arc::new(tokio::sync::Barrier::new(KEYS));

    let mut handles = Vec::new();
    for i in 0..KEYS {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            let key = format!("key-{i:03}");
            let body = format!("response-{i:03}").into_bytes();
            barrier.wait().await;

            assert_eq!(
                claim_guarded(&store, &key).await,
                ClaimOutcome::NewClaim,
                "distinct keys must all claim independently"
            );
            let clock = TestClock::new();
            store
                .complete(&key, &sample_response(&clock, 200, body.clone()))
                .await
                .expect("complete succeeds");

            // Immediate duplicate must replay this key's own body.
            match claim_guarded(&store, &key).await {
                ClaimOutcome::Completed(response) => {
                    assert_eq!(
                        response.body, body,
                        "key {key} must replay its own response"
                    );
                }
                other => panic!("key {key}: expected Completed, got {other:?}"),
            }
        }));
    }
    for handle in handles {
        handle.await.expect("key task panicked");
    }

    // After the storm: every key is independently replayable via get() and
    // claim(), with no cross-key leakage.
    for i in 0..KEYS {
        let key = format!("key-{i:03}");
        let expected = format!("response-{i:03}").into_bytes();

        let fetched = store
            .get(&key)
            .await
            .expect("get succeeds")
            .unwrap_or_else(|| panic!("get({key}) must return the stored response"));
        assert_eq!(
            fetched.body, expected,
            "get({key}) leaked another key's body"
        );

        match claim_guarded(&store, &key).await {
            ClaimOutcome::Completed(response) => {
                assert_eq!(
                    response.body, expected,
                    "claim({key}) leaked another key's body"
                );
            }
            other => panic!("key {key}: expected Completed, got {other:?}"),
        }
    }
}
