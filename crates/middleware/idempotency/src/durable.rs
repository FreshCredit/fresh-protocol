// TAG: surface=security owner=security-team rule=SEC-001
//! Durable, database-backed idempotency store.
//!
//! Persists idempotency records in the platform database (`idempotency_records`,
//! see `migrations/versioned/022_consistency_kernel.up.sql`) so that a keyed
//! request executes at most once even across process restarts and multiple API
//! replicas. Claims are atomic (`INSERT ... ON CONFLICT DO NOTHING`), crash
//! recovery reclaims stale `in_flight` rows, and completed responses are
//! replayed until their TTL expires.
//!
//! Wire it behind the middleware via [`crate::ResponseStore`] and
//! [`crate::IdempotencyLayer::with_store`]:
//!
//! ```ignore
//! let store = DurableIdempotencyStore::from_database(config, clock, platform_db).await?;
//! let layer = IdempotencyLayer::with_store(config, Arc::new(store));
//! ```

use std::sync::Arc;

use freshcredit_core_timing::Clock;
use tracing::{debug, info, warn};

use crate::config::IdempotencyConfig;
use crate::store::{IdempotencyError, StoredResponse};

/// Default scope recorded for claims made through the middleware
/// ([`crate::ResponseStore`] does not carry a scope).
const DEFAULT_SCOPE: &str = "http";

/// Lazily-applied DDL for `idempotency_records`; mirrors
/// `migrations/versioned/022_consistency_kernel.up.sql` so the store is
/// self-initializing when the migration runner has not reached version 022.
const CREATE_TABLE_DDL: &str = r"
CREATE TABLE IF NOT EXISTS idempotency_records (
    key         TEXT PRIMARY KEY,
    scope       TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'in_flight'
                CHECK (status IN ('in_flight', 'completed')),
    response    TEXT,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    completed_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_idempotency_records_status_created
    ON idempotency_records (status, created_at);
";

/// Errors returned by the durable idempotency store.
#[derive(Debug, thiserror::Error)]
#[allow(clippy::module_name_repetitions)]
pub enum DurableIdempotencyError {
    /// The platform database operation failed.
    #[error("durable idempotency store error: {0}")]
    Store(String),

    /// A stored response could not be serialized or deserialized.
    #[error("durable idempotency serialization error: {0}")]
    Serialization(String),
}

impl From<DurableIdempotencyError> for IdempotencyError {
    fn from(err: DurableIdempotencyError) -> Self {
        match err {
            DurableIdempotencyError::Store(detail)
            | DurableIdempotencyError::Serialization(detail) => Self::SerializationError(detail),
        }
    }
}

/// Outcome of attempting to claim an idempotency key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimOutcome {
    /// This caller inserted the claim and must execute the request.
    NewClaim,

    /// Another request holds the key and is still within the staleness window.
    InProgress,

    /// The key was already executed; replay this stored response.
    Completed(StoredResponse),
}

/// Durable idempotency store backed by the platform database.
///
/// Uses only single-statement atomic writes, so claims are safe across
/// concurrent replicas sharing one primary (single-writer quorum; see
/// `docs/service/SYSTEM_MODEL.md`).
pub struct DurableIdempotencyStore {
    /// Platform database handle (kept alive for the connection's lifetime)
    _db: Arc<libsql::Database>,

    /// Connection used for all record operations
    conn: Arc<libsql::Connection>,

    /// Response TTL, maximum body size, and claim staleness timeout
    config: IdempotencyConfig,

    /// Clock used for response-expiry checks
    clock: Arc<dyn Clock>,

    /// Scope written into new claim rows
    scope: String,
}

impl std::fmt::Debug for DurableIdempotencyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DurableIdempotencyStore")
            .field("config", &self.config)
            .field("scope", &self.scope)
            .field("conn", &"<libsql::Connection>")
            .finish_non_exhaustive()
    }
}

impl DurableIdempotencyStore {
    /// Create a durable store from the platform database handle.
    ///
    /// Connects, applies `CREATE TABLE IF NOT EXISTS` for
    /// `idempotency_records` (idempotent mirror of migration 022), and returns
    /// a ready store.
    ///
    /// # Errors
    ///
    /// Returns an error if connecting to the database or applying the
    /// table DDL fails.
    pub async fn from_database(
        config: IdempotencyConfig,
        clock: Arc<dyn Clock>,
        db: Arc<libsql::Database>,
    ) -> Result<Self, DurableIdempotencyError> {
        let conn = db
            .connect()
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;
        let conn = Arc::new(conn);

        conn.execute_batch(CREATE_TABLE_DDL)
            .await
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;

        Ok(Self {
            _db: db,
            conn,
            config,
            clock,
            scope: DEFAULT_SCOPE.to_string(),
        })
    }

    /// Set the scope recorded on new claim rows (defaults to `"http"`).
    #[must_use]
    pub fn with_scope(mut self, scope: String) -> Self {
        self.scope = scope;
        self
    }

    /// Atomically claim an idempotency key.
    ///
    /// - New key: inserts an `in_flight` row and returns [`ClaimOutcome::NewClaim`].
    /// - Existing `completed` row: returns [`ClaimOutcome::Completed`] with the
    ///   stored response, unless it has expired, in which case the row is
    ///   reclaimed and [`ClaimOutcome::NewClaim`] is returned.
    /// - Existing `in_flight` row younger than the staleness timeout returns
    ///   [`ClaimOutcome::InProgress`]; an `in_flight` row older than the
    ///   staleness timeout is atomically reclaimed (crash recovery) and
    ///   [`ClaimOutcome::NewClaim`] is returned.
    ///
    /// # Errors
    ///
    /// Returns an error if the platform database operation or response
    /// deserialization fails.
    pub async fn claim(
        &self,
        key: &str,
        scope: &str,
    ) -> Result<ClaimOutcome, DurableIdempotencyError> {
        let inserted = self
            .conn
            .execute(
                "INSERT INTO idempotency_records(key, scope, status) \
                 VALUES(?1, ?2, 'in_flight') ON CONFLICT(key) DO NOTHING",
                libsql::params![key, scope],
            )
            .await
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;

        if inserted == 1 {
            debug!("Durable idempotency claim inserted: {}", key);
            return Ok(ClaimOutcome::NewClaim);
        }

        self.classify_existing(key, scope).await
    }

    /// Read and classify the existing row for `key` after an
    /// `ON CONFLICT DO NOTHING` insert found one.
    async fn classify_existing(
        &self,
        key: &str,
        scope: &str,
    ) -> Result<ClaimOutcome, DurableIdempotencyError> {
        let mut rows = self
            .conn
            .query(
                "SELECT status, response, created_at FROM idempotency_records WHERE key = ?1",
                libsql::params![key],
            )
            .await
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;

        let row = rows
            .next()
            .await
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?
            .ok_or_else(|| {
                DurableIdempotencyError::Store(format!("claim for key {key} vanished mid-claim"))
            })?;

        let status: String = row
            .get(0)
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;

        match status.as_str() {
            "completed" => self.classify_completed(key, scope, row).await,
            "in_flight" => self.classify_in_flight(key, row).await,
            other => Err(DurableIdempotencyError::Store(format!(
                "idempotency record for key {key} has invalid status {other:?}"
            ))),
        }
    }

    /// Classify an existing `completed` row; expired rows are reclaimed so a
    /// finished response can never be replayed past its TTL.
    async fn classify_completed(
        &self,
        key: &str,
        scope: &str,
        row: libsql::Row,
    ) -> Result<ClaimOutcome, DurableIdempotencyError> {
        let response_json: String = row
            .get(1)
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;
        let response = Self::deserialize_response(key, &response_json)?;

        if !response.is_expired(&*self.clock) {
            debug!("Durable idempotency replay: {}", key);
            return Ok(ClaimOutcome::Completed(response));
        }

        // Expired: reclaim the row for a fresh execution.
        let deleted = self
            .conn
            .execute(
                "DELETE FROM idempotency_records WHERE key = ?1 AND status = 'completed'",
                libsql::params![key],
            )
            .await
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;

        if deleted == 1 {
            info!("Reclaimed expired durable idempotency record: {}", key);
            return self.claim_fresh(key, scope).await;
        }

        // Lost a reclaim race; treat as in progress rather than execute twice.
        warn!("Lost expired-record reclaim race for key {}", key);
        Ok(ClaimOutcome::InProgress)
    }

    /// Classify an existing `in_flight` row; stale rows (held longer than the
    /// staleness timeout) are atomically reclaimed so a crashed holder cannot
    /// wedge the key forever.
    async fn classify_in_flight(
        &self,
        key: &str,
        row: libsql::Row,
    ) -> Result<ClaimOutcome, DurableIdempotencyError> {
        let created_at: i64 = row
            .get(2)
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;
        let now_unix = self.unix_now().await?;
        let stale_before = now_unix - i64::try_from(self.config.stale_seconds).unwrap_or(i64::MAX);

        if created_at >= stale_before {
            debug!("Durable idempotency key in progress: {}", key);
            return Ok(ClaimOutcome::InProgress);
        }

        // The holder may have crashed mid-request: reset its claim. The
        // conditional `status = 'in_flight'` predicate makes the reclaim
        // atomic, so exactly one racing caller wins.
        let updated = self
            .conn
            .execute(
                "UPDATE idempotency_records \
                 SET status = 'in_flight', created_at = unixepoch() \
                 WHERE key = ?1 AND status = 'in_flight'",
                libsql::params![key],
            )
            .await
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;

        if updated == 1 {
            warn!(
                "Reclaimed stale in-flight durable idempotency claim: {}",
                key
            );
            return Ok(ClaimOutcome::NewClaim);
        }

        // Lost the reclaim race to another caller; its fresh claim stands.
        debug!("Lost stale-claim reclaim race for key {}", key);
        Ok(ClaimOutcome::InProgress)
    }

    /// Insert a fresh `in_flight` row, assuming the caller already ensured no
    /// row exists (reclaim path). On a conflict race, classify the winner.
    async fn claim_fresh(
        &self,
        key: &str,
        scope: &str,
    ) -> Result<ClaimOutcome, DurableIdempotencyError> {
        let inserted = self
            .conn
            .execute(
                "INSERT INTO idempotency_records(key, scope, status) \
                 VALUES(?1, ?2, 'in_flight') ON CONFLICT(key) DO NOTHING",
                libsql::params![key, scope],
            )
            .await
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;

        if inserted == 1 {
            Ok(ClaimOutcome::NewClaim)
        } else {
            // Lost a reclaim race to another caller; its claim stands.
            debug!("Lost fresh-claim race for key {}", key);
            Ok(ClaimOutcome::InProgress)
        }
    }

    /// Mark a claimed key as completed with its response payload.
    ///
    /// Only an `in_flight` row owned by the current claim is updated: a stale
    /// holder that lost its claim to a reclaim can never overwrite the new
    /// holder's completed response.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails, the database update fails, or
    /// the claim was already completed or reclaimed by another caller.
    pub async fn complete(
        &self,
        key: &str,
        response: &StoredResponse,
    ) -> Result<(), DurableIdempotencyError> {
        let response_json = serde_json::to_string(response)
            .map_err(|e| DurableIdempotencyError::Serialization(e.to_string()))?;

        let updated = self
            .conn
            .execute(
                "UPDATE idempotency_records \
                 SET status = 'completed', response = ?2, completed_at = unixepoch() \
                 WHERE key = ?1 AND status = 'in_flight'",
                libsql::params![key, response_json],
            )
            .await
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;

        if updated == 0 {
            // The claim was reclaimed and re-owned (or never existed); a
            // completed record must never be overwritten by a stale holder.
            return Err(DurableIdempotencyError::Store(format!(
                "idempotency claim for key {key} was lost or already completed"
            )));
        }

        info!("Completed durable idempotency record: {}", key);
        Ok(())
    }

    /// Read a completed response for `key`, honoring its expiry.
    ///
    /// Returns `None` if the record is missing, still `in_flight`, or expired.
    ///
    /// # Errors
    ///
    /// Returns an error if the database read or response deserialization fails.
    pub async fn get(&self, key: &str) -> Result<Option<StoredResponse>, DurableIdempotencyError> {
        let mut rows = self
            .conn
            .query(
                "SELECT response FROM idempotency_records \
                 WHERE key = ?1 AND status = 'completed'",
                libsql::params![key],
            )
            .await
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;

        let Some(row) = rows
            .next()
            .await
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?
        else {
            return Ok(None);
        };

        let response_json: String = row
            .get(0)
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;
        let response = Self::deserialize_response(key, &response_json)?;

        if response.is_expired(&*self.clock) {
            debug!("Durable idempotency response expired: {}", key);
            return Ok(None);
        }

        Ok(Some(response))
    }

    /// Build, size-check, and durably store a response for `key`.
    ///
    /// # Errors
    ///
    /// Returns [`IdempotencyError::BodyTooLarge`] if the body exceeds the
    /// configured maximum, or an error if persisting the response fails.
    pub async fn store_response(
        &self,
        key: &str,
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> Result<StoredResponse, IdempotencyError> {
        if body.len() > self.config.max_body_size {
            return Err(IdempotencyError::BodyTooLarge {
                size: body.len(),
                max: self.config.max_body_size,
            });
        }

        let response = StoredResponse::new(status, headers, body, self.config.ttl, &*self.clock);
        self.complete(key, &response).await?;
        Ok(response)
    }

    /// Current unix time (seconds) from the database clock, matching the
    /// `unixepoch()` semantics of `created_at`/`completed_at`.
    async fn unix_now(&self) -> Result<i64, DurableIdempotencyError> {
        let mut rows = self
            .conn
            .query("SELECT unixepoch()", ())
            .await
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?;

        let row = rows
            .next()
            .await
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))?
            .ok_or_else(|| DurableIdempotencyError::Store("unixepoch() returned no row".into()))?;

        row.get(0)
            .map_err(|e| DurableIdempotencyError::Store(e.to_string()))
    }

    fn deserialize_response(
        key: &str,
        response_json: &str,
    ) -> Result<StoredResponse, DurableIdempotencyError> {
        serde_json::from_str(response_json).map_err(|e| {
            DurableIdempotencyError::Serialization(format!(
                "corrupt stored response for key {key}: {e}"
            ))
        })
    }
}

impl crate::ResponseStore for DurableIdempotencyStore {
    async fn lookup(&self, key: &str) -> Result<crate::LookupOutcome, IdempotencyError> {
        match self.claim(key, &self.scope).await? {
            ClaimOutcome::NewClaim => Ok(crate::LookupOutcome::Proceed),
            ClaimOutcome::InProgress => Ok(crate::LookupOutcome::Conflict),
            ClaimOutcome::Completed(response) => Ok(crate::LookupOutcome::Replay(response)),
        }
    }

    async fn store_response(
        &self,
        key: &str,
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> Result<StoredResponse, IdempotencyError> {
        Self::store_response(self, key, status, headers, body).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;
    use std::time::Duration;

    use freshcredit_core_timing::MockClock;

    use super::*;

    /// Interior-mutable test clock (mirrors the wrapper in `store.rs` tests).
    struct TestClock {
        inner: StdMutex<MockClock>,
    }

    impl TestClock {
        fn new(time: chrono::DateTime<chrono::Utc>) -> Self {
            Self {
                inner: StdMutex::new(MockClock::new(time)),
            }
        }

        fn advance(&self, duration: chrono::Duration) {
            self.inner.lock().unwrap().advance(duration);
        }
    }

    impl Clock for TestClock {
        fn now(&self) -> chrono::DateTime<chrono::Utc> {
            self.inner.lock().unwrap().now()
        }
    }

    async fn create_store(
        ttl: Duration,
        stale_seconds: u64,
    ) -> (Arc<DurableIdempotencyStore>, Arc<TestClock>) {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .expect("in-memory db builds");
        let clock = Arc::new(TestClock::new(chrono::Utc::now()));
        let config = IdempotencyConfig::new(ttl).with_stale_seconds(stale_seconds);
        let store = DurableIdempotencyStore::from_database(config, clock.clone(), Arc::new(db))
            .await
            .expect("store constructs");
        (Arc::new(store), clock)
    }

    fn sample_response(clock: &TestClock, ttl: Duration) -> StoredResponse {
        StoredResponse::new(
            200,
            vec![("content-type".to_string(), "application/json".to_string())],
            vec![1, 2, 3],
            ttl,
            clock,
        )
    }

    #[tokio::test]
    async fn claim_new_key_returns_new_claim() {
        let (store, _clock) = create_store(Duration::from_secs(3600), 300).await;

        let outcome = store.claim("key1", "test").await.unwrap();
        assert_eq!(outcome, ClaimOutcome::NewClaim);
    }

    #[tokio::test]
    async fn second_claim_is_in_progress() {
        let (store, _clock) = create_store(Duration::from_secs(3600), 300).await;

        assert_eq!(
            store.claim("key1", "test").await.unwrap(),
            ClaimOutcome::NewClaim
        );
        assert_eq!(
            store.claim("key1", "test").await.unwrap(),
            ClaimOutcome::InProgress
        );
    }

    #[tokio::test]
    async fn complete_then_claim_replays() {
        let (store, clock) = create_store(Duration::from_secs(3600), 300).await;

        assert_eq!(
            store.claim("key1", "test").await.unwrap(),
            ClaimOutcome::NewClaim
        );
        store
            .complete("key1", &sample_response(&clock, Duration::from_secs(3600)))
            .await
            .unwrap();

        let outcome = store.claim("key1", "test").await.unwrap();
        match outcome {
            ClaimOutcome::Completed(replayed) => {
                assert_eq!(replayed.status, 200);
                assert_eq!(replayed.body, vec![1, 2, 3]);
                assert_eq!(
                    replayed.headers,
                    vec![("content-type".to_string(), "application/json".to_string())]
                );
            }
            other => panic!("expected Completed replay, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_returns_none_until_completed() {
        let (store, clock) = create_store(Duration::from_secs(3600), 300).await;

        assert!(store.get("key1").await.unwrap().is_none());

        store.claim("key1", "test").await.unwrap();
        assert!(store.get("key1").await.unwrap().is_none());

        store
            .complete("key1", &sample_response(&clock, Duration::from_secs(3600)))
            .await
            .unwrap();
        let response = store.get("key1").await.unwrap();
        assert!(response.is_some());
        assert_eq!(response.unwrap().status, 200);
    }

    #[tokio::test]
    async fn get_returns_none_for_missing_key() {
        let (store, _clock) = create_store(Duration::from_secs(3600), 300).await;
        assert!(store.get("missing").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn expired_response_returns_none_and_is_reclaimed() {
        let (store, clock) = create_store(Duration::from_secs(60), 300).await;

        store.claim("key1", "test").await.unwrap();
        store
            .complete("key1", &sample_response(&clock, Duration::from_secs(60)))
            .await
            .unwrap();
        assert!(store.get("key1").await.unwrap().is_some());

        // Advance past the response TTL: replay must stop.
        clock.advance(chrono::Duration::seconds(61));
        assert!(store.get("key1").await.unwrap().is_none());

        // A new claim must be allowed (the expired record is reclaimed).
        let outcome = store.claim("key1", "test").await.unwrap();
        assert_eq!(outcome, ClaimOutcome::NewClaim);
    }

    #[tokio::test]
    async fn stale_in_flight_claim_is_reclaimed() {
        // Backdate the claim's created_at beyond the 300s staleness window to
        // simulate a holder that crashed without completing.
        let (store, _clock) = create_store(Duration::from_secs(3600), 300).await;

        assert_eq!(
            store.claim("key1", "test").await.unwrap(),
            ClaimOutcome::NewClaim
        );

        store
            .conn
            .execute(
                "UPDATE idempotency_records SET created_at = created_at - 400 WHERE key = 'key1'",
                (),
            )
            .await
            .unwrap();

        assert_eq!(
            store.claim("key1", "test").await.unwrap(),
            ClaimOutcome::NewClaim
        );
    }

    #[tokio::test]
    async fn complete_never_overwrites_a_completed_record() {
        // A stale holder that lost its claim must not clobber the new holder's
        // completed response (zombie protection).
        let (store, clock) = create_store(Duration::from_secs(3600), 300).await;

        assert_eq!(
            store.claim("key1", "test").await.unwrap(),
            ClaimOutcome::NewClaim
        );
        store
            .complete("key1", &sample_response(&clock, Duration::from_secs(3600)))
            .await
            .unwrap();

        let err = store
            .complete("key1", &sample_response(&clock, Duration::from_secs(3600)))
            .await
            .unwrap_err();
        assert!(matches!(err, DurableIdempotencyError::Store(_)));

        // The original response still replays.
        let outcome = store.claim("key1", "test").await.unwrap();
        match outcome {
            ClaimOutcome::Completed(response) => assert_eq!(response.body, vec![1, 2, 3]),
            other => panic!("expected Completed replay, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn fresh_in_flight_claim_is_not_reclaimed() {
        // Default staleness window: a concurrent holder is honored.
        let (store, _clock) = create_store(Duration::from_secs(3600), 300).await;

        assert_eq!(
            store.claim("key1", "test").await.unwrap(),
            ClaimOutcome::NewClaim
        );
        assert_eq!(
            store.claim("key1", "test").await.unwrap(),
            ClaimOutcome::InProgress
        );
    }

    #[tokio::test]
    async fn concurrent_claims_have_exactly_one_winner() {
        const TASKS: usize = 8;

        let (store, _clock) = create_store(Duration::from_secs(3600), 300).await;

        let outcomes = Arc::new(StdMutex::new(Vec::new()));
        let mut handles = Vec::new();
        for _ in 0..TASKS {
            let store = Arc::clone(&store);
            let outcomes = Arc::clone(&outcomes);
            handles.push(tokio::spawn(async move {
                let outcome = store.claim("hot-key", "test").await.unwrap();
                outcomes.lock().unwrap().push(outcome);
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }

        let (new_claims, in_progress) = {
            let outcomes = outcomes.lock().unwrap();
            let new_claims = outcomes
                .iter()
                .filter(|o| **o == ClaimOutcome::NewClaim)
                .count();
            let in_progress = outcomes
                .iter()
                .filter(|o| **o == ClaimOutcome::InProgress)
                .count();
            drop(outcomes);
            (new_claims, in_progress)
        };
        assert_eq!(new_claims, 1, "exactly one winner");
        assert_eq!(in_progress, TASKS - 1);
    }

    #[tokio::test]
    async fn response_store_lookup_flow() {
        use crate::{LookupOutcome, ResponseStore};

        let (store, clock) = create_store(Duration::from_secs(3600), 300).await;

        // New key -> Proceed.
        let outcome = ResponseStore::lookup(&*store, "key1").await.unwrap();
        assert_eq!(outcome, LookupOutcome::Proceed);

        // The first claim is uncompleted, so a second lookup conflicts.
        let outcome = ResponseStore::lookup(&*store, "key1").await.unwrap();
        assert_eq!(outcome, LookupOutcome::Conflict);

        // Claim, store, then replay.
        assert_eq!(
            store.claim("key2", "test").await.unwrap(),
            ClaimOutcome::NewClaim
        );
        let stored = store
            .store_response("key2", 201, vec![], vec![9, 9])
            .await
            .unwrap();
        assert_eq!(stored.status, 201);
        let outcome = ResponseStore::lookup(&*store, "key2").await.unwrap();
        match outcome {
            LookupOutcome::Replay(response) => assert_eq!(response.body, vec![9, 9]),
            other => panic!("expected Replay, got {other:?}"),
        }

        // Expired replay -> the record is reclaimed and the caller proceeds.
        clock.advance(chrono::Duration::seconds(3601));
        let outcome = ResponseStore::lookup(&*store, "key2").await.unwrap();
        assert_eq!(outcome, LookupOutcome::Proceed);

        // Body size limit enforced.
        let big = vec![0_u8; (1024 * 1024) + 1];
        let err = store
            .store_response("key3", 200, vec![], big)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            IdempotencyError::BodyTooLarge { size, max }
                if size == (1024 * 1024) + 1 && max == 1024 * 1024
        ));
    }
}
