//! Consistency kernel: durable job leases with fencing tokens and a
//! transactional outbox relay.
//!
//! This crate is the single place that implements the distributed-systems
//! primitives the rest of the platform relies on (see
//! `docs/service/SYSTEM_MODEL.md`): partial synchrony with crash-recovery is
//! the system model, so correctness cannot depend on "only one replica runs
//! this job" (Kubernetes maxScale=1 is a scheduling hint, not a guarantee).
//!
//! # Leases and fencing tokens
//!
//! [`Consistency::claim_lease`] atomically claims a named job lease in the
//! platform database. Each successful claim increments a persisted `epoch`
//! (the fencing token). Any writer that receives a [`FencingToken`] must
//! validate it with [`Consistency::validate_fencing`] before committing, so a
//! paused/stale holder that lost its lease cannot commit work after a new
//! holder has taken over.
//!
//! All expiry arithmetic uses the database's own `unixepoch('subsec')` clock,
//! never the application host's wall clock. The platform database is the
//! single time authority, so lease expiry decisions cannot be skewed by clock
//! drift between application replicas (a replica with a fast clock cannot
//! steal a live lease, and a replica with a slow clock cannot extend its
//! own). Sub-second precision keeps a lease valid for its full TTL no matter
//! where within the current second it was claimed.
//!
//! # Transactional outbox
//!
//! [`enqueue_outbox`] inserts an event into the `outbox` table **in the
//! caller's transaction**, so the event is durably recorded exactly when the
//! state change commits. [`run_outbox_relay`] polls unpublished rows and
//! delivers them through an [`OutboxPublisher`] sink, stamping `published_at`
//! only after the sink acknowledges.

use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// Lazily-applied DDL for `job_leases`; mirrors
/// `crates/consistency/tests/migrations/022_consistency_kernel.up.sql` so the kernel is
/// self-initializing when the migration runner has not reached version 022.
const JOB_LEASES_DDL: &str = r"
CREATE TABLE IF NOT EXISTS job_leases (
    name        TEXT PRIMARY KEY,
    epoch       INTEGER NOT NULL DEFAULT 0,
    holder      TEXT NOT NULL,
    expires_at  INTEGER NOT NULL
);
";

/// Lazily-applied DDL for the transactional outbox (+ its partial index);
/// mirrors `crates/consistency/tests/migrations/022_consistency_kernel.up.sql` so the relay
/// is self-initializing when the migration runner has not reached version 022.
const OUTBOX_DDL: &str = r"
CREATE TABLE IF NOT EXISTS outbox (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    topic        TEXT NOT NULL,
    payload      TEXT NOT NULL,
    created_at   INTEGER NOT NULL DEFAULT (unixepoch()),
    published_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_outbox_unpublished
    ON outbox (published_at) WHERE published_at IS NULL;
";

/// Errors returned by the consistency kernel.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The platform database operation failed.
    #[error("consistency store error: {0}")]
    Store(String),

    /// The outbox publisher rejected an event; the relay will retry.
    #[error("outbox publisher error (topic {topic}): {detail}")]
    Publish {
        /// Outbox topic that failed to publish.
        topic: String,
        /// Underlying error, serialized.
        detail: String,
    },
}

/// A fencing token proving the holder currently owns a named lease.
///
/// Epochs are persisted in the `job_leases` table and strictly increase on
/// every successful claim, including claims after a holder crashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencingToken {
    /// Logical job name (primary key of `job_leases`).
    pub name: String,

    /// Monotonically increasing fencing epoch for `name`.
    pub epoch: i64,

    /// Unique holder instance id that was granted this token.
    pub holder: String,
}

/// Sinks that deliver outbox events to the outside world.
#[async_trait::async_trait]
pub trait OutboxPublisher: Send + Sync {
    /// Publish one event. Returning `Err` makes the relay keep the row
    /// unpublished and retry on the next poll.
    ///
    /// Implementations must be idempotent: the relay guarantees at-least-once
    /// delivery, and a crash after the sink acknowledges but before
    /// `published_at` is stamped can cause a redelivery.
    ///
    /// # Errors
    ///
    /// Returns an error if the event could not be delivered; the event stays
    /// unpublished and will be retried.
    async fn publish(&self, topic: &str, payload: &str) -> Result<(), Error>;
}

/// Durable leases + outbox administration backed by the platform database.
///
/// A single connection to the platform database is established lazily on the
/// first operation and reused afterwards, matching the long-lived
/// `Arc<libsql::Connection>` pattern used by the platform's other services.
/// (For an in-memory local database, each `Database::connect()` opens a
/// *fresh* empty database, so one connection per `Consistency` instance is
/// also what makes in-memory operation behave intuitively.)
#[derive(Clone)]
pub struct Consistency {
    db: Arc<libsql::Database>,
    conn: OnceLock<libsql::Connection>,
}

impl std::fmt::Debug for Consistency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Consistency").finish_non_exhaustive()
    }
}

impl Consistency {
    /// Create a kernel handle over the platform database.
    ///
    /// No I/O happens here; the first operation connects lazily and, before
    /// serving it, idempotently ensures the `job_leases` table exists
    /// (defensive mirror of migration `022_consistency_kernel`, matching the
    /// durable idempotency store's self-initializing pattern — logged at
    /// `info!` when the table had to be created, `debug!` otherwise). The
    /// `outbox` table is likewise ensured by [`run_outbox_relay`] before its
    /// first poll.
    #[must_use]
    pub const fn new(db: Arc<libsql::Database>) -> Self {
        Self {
            db,
            conn: OnceLock::new(),
        }
    }

    /// Atomically claim the named lease.
    ///
    /// Returns `Ok(None)` when the lease is held by another holder whose
    /// lease has not expired. Returns `Ok(Some(token))` when the caller now
    /// holds the lease; `token.epoch` is the new fencing epoch.
    ///
    /// The claim is a single atomic `INSERT ... ON CONFLICT(name) DO UPDATE
    /// ... WHERE job_leases.expires_at < unixepoch('subsec') RETURNING ...`
    /// statement, so concurrent claims on any number of replicas serialize on
    /// the platform database primary. `expires_at` is computed with the
    /// database's own `unixepoch('subsec')` (see the module docs for why), so
    /// a lease is valid for its full TTL regardless of when within the
    /// current second it was claimed.
    ///
    /// # Errors
    ///
    /// Returns an error if the platform database is unreachable.
    pub async fn claim_lease(
        &self,
        name: &str,
        holder: &str,
        ttl: Duration,
    ) -> Result<Option<FencingToken>, Error> {
        let ttl_secs = ttl.as_secs_f64();
        let mut rows = self
            .connection()
            .await?
            .query(
                "INSERT INTO job_leases (name, epoch, holder, expires_at)
                 VALUES (?1, 1, ?2, unixepoch('subsec') + ?3)
                 ON CONFLICT(name) DO UPDATE SET
                     epoch = job_leases.epoch + 1,
                     holder = excluded.holder,
                     expires_at = excluded.expires_at
                 WHERE job_leases.expires_at < unixepoch('subsec')
                 RETURNING epoch, holder",
                libsql::params![name, holder, ttl_secs],
            )
            .await
            .map_err(|e| Error::Store(format!("claim lease {name}: {e}")))?;

        match rows
            .next()
            .await
            .map_err(|e| Error::Store(format!("claim lease {name}: {e}")))?
        {
            Some(row) => {
                let epoch: i64 = row
                    .get(0)
                    .map_err(|e| Error::Store(format!("claim lease {name}: {e}")))?;
                let holder: String = row
                    .get(1)
                    .map_err(|e| Error::Store(format!("claim lease {name}: {e}")))?;
                Ok(Some(FencingToken {
                    name: name.to_string(),
                    epoch,
                    holder,
                }))
            }
            None => Ok(None),
        }
    }

    /// Check whether a fencing token is still the current one for its lease.
    ///
    /// Writers call this immediately before committing side effects gated on
    /// a lease; `false` means a newer holder took over and the write must be
    /// abandoned.
    ///
    /// The check reads `epoch`, `expires_at`, and the database's
    /// `unixepoch('subsec')` in a single `SELECT`, so all three values come
    /// from one consistent snapshot of the platform database. `expires_at` is
    /// cast to REAL because released rows store `0` (INTEGER) while live rows
    /// store fractional timestamps.
    ///
    /// # Errors
    ///
    /// Returns an error if the platform database is unreachable.
    pub async fn validate_fencing(&self, token: &FencingToken) -> Result<bool, Error> {
        let mut rows = self
            .connection()
            .await?
            .query(
                "SELECT epoch, CAST(expires_at AS REAL), unixepoch('subsec') AS now
                 FROM job_leases WHERE name = ?1",
                libsql::params![token.name.as_str()],
            )
            .await
            .map_err(|e| Error::Store(format!("validate fencing {}: {e}", token.name)))?;

        let Some(row) = rows
            .next()
            .await
            .map_err(|e| Error::Store(format!("validate fencing {}: {e}", token.name)))?
        else {
            return Ok(false);
        };
        let epoch: i64 = row
            .get(0)
            .map_err(|e| Error::Store(format!("validate fencing {}: {e}", token.name)))?;
        let expires_at: f64 = row
            .get(1)
            .map_err(|e| Error::Store(format!("validate fencing {}: {e}", token.name)))?;
        let now: f64 = row
            .get(2)
            .map_err(|e| Error::Store(format!("validate fencing {}: {e}", token.name)))?;
        Ok(epoch == token.epoch && expires_at > now)
    }

    /// Release the named lease if (and only if) `holder` currently holds it.
    ///
    /// Idempotent: a no-op when the lease is absent or held by somebody else.
    /// Expiry is set to `0` (always in the past for any live database clock),
    /// so the lease becomes claimable by any holder in the very next claim
    /// statement — `unixepoch()` has whole-second granularity, so stamping
    /// the current second could make an immediate re-claim wait one second.
    ///
    /// # Errors
    ///
    /// Returns an error if the platform database is unreachable.
    pub async fn release_lease(&self, name: &str, holder: &str) -> Result<(), Error> {
        self.connection()
            .await?
            .execute(
                "UPDATE job_leases SET expires_at = 0
                 WHERE name = ?1 AND holder = ?2",
                libsql::params![name, holder],
            )
            .await
            .map_err(|e| Error::Store(format!("release lease {name}: {e}")))?;
        Ok(())
    }

    /// Connection accessor for the public async operations; establishes the
    /// connection lazily on first use, idempotently ensuring the `job_leases`
    /// table exists (defensive mirror of migration 022) before any lease
    /// operation can fail on a missing table.
    async fn connection(&self) -> Result<&libsql::Connection, Error> {
        if self.conn.get().is_none() {
            let conn = self
                .db
                .connect()
                .map_err(|e| Error::Store(format!("connect to platform database: {e}")))?;
            ensure_table(&conn, "job_leases", JOB_LEASES_DDL).await?;
            // If another task initialized concurrently, keep the existing
            // connection; `libsql::Connection` handles are cheap to clone but
            // one shared connection keeps in-memory databases consistent.
            let _ = self.conn.set(conn);
        }
        Ok(self
            .conn
            .get()
            .expect("connection initialized above or by a concurrent caller"))
    }
}

/// Insert an outbox event in the caller's transaction.
///
/// Call this inside the same transaction as the state change the event
/// announces; the event becomes durable exactly when the state change does.
///
/// # Errors
///
/// Returns an error if the insert fails; the caller should roll back.
pub async fn enqueue_outbox(
    tx: &libsql::Transaction,
    topic: &str,
    payload: &str,
) -> Result<i64, Error> {
    tx.execute(
        "INSERT INTO outbox (topic, payload) VALUES (?1, ?2)",
        libsql::params![topic, payload],
    )
    .await
    .map_err(|e| Error::Store(format!("enqueue outbox event (topic {topic}): {e}")))?;
    Ok(tx.last_insert_rowid())
}

/// Poll and deliver unpublished outbox rows.
///
/// Loops forever: claim a batch of unpublished rows, deliver each through
/// `publisher`, stamp `published_at` for acknowledged rows, then sleep
/// `poll_interval`. At-least-once delivery; ordering is by insertion id
/// within a batch but not guaranteed across batches.
///
/// Returns only when `shutdown` is signaled; individual delivery failures are
/// logged and retried on the next poll.
///
/// # Errors
///
/// Returns an error only on unrecoverable startup failure (e.g. the platform
/// database is unreachable on the first poll).
pub async fn run_outbox_relay(
    db: Arc<libsql::Database>,
    publisher: Arc<dyn OutboxPublisher>,
    poll_interval: Duration,
    batch_size: u32,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), Error> {
    let conn = db
        .connect()
        .map_err(|e| Error::Store(format!("outbox relay connect: {e}")))?;
    // Self-initialize before the first poll: if the migration runner has not
    // reached version 022, create the outbox table (+ partial index) instead
    // of failing the first SELECT. Log at info when created, debug otherwise.
    ensure_table(&conn, "outbox", OUTBOX_DDL).await?;

    let mut first_poll = true;
    loop {
        if *shutdown.borrow() {
            break;
        }

        let batch = match fetch_unpublished_batch(&conn, batch_size).await {
            Ok(batch) => batch,
            Err(e) => {
                // A dead platform database at startup is fatal; once running,
                // transient failures are logged and retried after the poll
                // interval, matching per-delivery failure handling.
                if first_poll {
                    return Err(e);
                }
                tracing::warn!("outbox relay poll failed, retrying: {e}");
                Vec::new()
            }
        };
        first_poll = false;

        for (id, topic, payload) in batch {
            match publisher.publish(&topic, &payload).await {
                Ok(()) => {
                    // The `published_at IS NULL` guard makes this a no-op if a
                    // concurrent relay already stamped the row, so
                    // redelivery-after-ack is safe.
                    if let Err(e) = conn
                        .execute(
                            "UPDATE outbox SET published_at = unixepoch()
                             WHERE id = ?1 AND published_at IS NULL",
                            libsql::params![id],
                        )
                        .await
                    {
                        tracing::warn!("outbox relay failed to stamp row {id}: {e}");
                    }
                }
                Err(e) => {
                    tracing::warn!("outbox relay publish failed (topic {topic}): {e}");
                }
            }
        }

        tokio::select! {
            _ = shutdown.changed() => break,
            () = tokio::time::sleep(poll_interval) => {}
        }
    }

    Ok(())
}

/// Idempotently ensure `table` exists, applying `ddl` (`CREATE ... IF NOT
/// EXISTS` statements mirroring migration 022) when it does not.
///
/// Logs at `info!` when the table had to be created (the migration runner has
/// not reached version 022) and at `debug!` when it already existed — the
/// self-initializing pattern established by the durable idempotency store.
///
/// # Errors
///
/// Returns an error if the existence probe or the DDL application fails.
async fn ensure_table(conn: &libsql::Connection, table: &str, ddl: &str) -> Result<(), Error> {
    let mut rows = conn
        .query(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            libsql::params![table],
        )
        .await
        .map_err(|e| Error::Store(format!("probe table {table}: {e}")))?;
    let existed = rows
        .next()
        .await
        .map_err(|e| Error::Store(format!("probe table {table}: {e}")))?
        .is_some();
    if existed {
        tracing::debug!("consistency table {table} already present");
        return Ok(());
    }
    conn.execute_batch(ddl)
        .await
        .map_err(|e| Error::Store(format!("create consistency table {table}: {e}")))?;
    tracing::info!(
        "created consistency table {table} (defensive bootstrap; migration 022_consistency_kernel not applied)"
    );
    Ok(())
}

async fn fetch_unpublished_batch(
    conn: &libsql::Connection,
    batch_size: u32,
) -> Result<Vec<(i64, String, String)>, Error> {
    let limit = i64::from(batch_size);
    let mut rows = conn
        .query(
            "SELECT id, topic, payload FROM outbox
             WHERE published_at IS NULL
             ORDER BY id ASC
             LIMIT ?1",
            libsql::params![limit],
        )
        .await
        .map_err(|e| Error::Store(format!("outbox relay fetch batch: {e}")))?;

    let mut batch = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| Error::Store(format!("outbox relay fetch batch: {e}")))?
    {
        let id: i64 = row
            .get(0)
            .map_err(|e| Error::Store(format!("outbox relay fetch batch: {e}")))?;
        let topic: String = row
            .get(1)
            .map_err(|e| Error::Store(format!("outbox relay fetch batch: {e}")))?;
        let payload: String = row
            .get(2)
            .map_err(|e| Error::Store(format!("outbox relay fetch batch: {e}")))?;
        batch.push((id, topic, payload));
    }
    Ok(batch)
}

/// Run `job` under a durable lease with fencing.
///
/// Claims `name` for `holder`; if the claim succeeds, invokes
/// `job(token)` and releases the lease afterwards. If the claim fails
/// (another live holder), returns `Ok(None)` without running the job.
///
/// `job` is responsible for calling [`Consistency::validate_fencing`] before
/// any commit that must not be duplicated across holders.
///
/// # Errors
///
/// Returns an error if the lease cannot be claimed or released due to store
/// failures. Errors from `job` itself are returned to the caller verbatim; on
/// a job error the lease is intentionally left to expire naturally (its TTL),
/// so a failed holder does not immediately invite a concurrent retry that
/// could interleave with its partial side effects.
pub async fn run_with_lease<F, Fut>(
    consistency: &Consistency,
    name: &str,
    holder: &str,
    ttl: Duration,
    job: F,
) -> Result<Option<FencingToken>, Error>
where
    F: FnOnce(FencingToken) -> Fut,
    Fut: Future<Output = Result<(), Error>>,
{
    let Some(token) = consistency.claim_lease(name, holder, ttl).await? else {
        return Ok(None);
    };
    job(token.clone()).await?;
    if let Err(e) = consistency.release_lease(name, holder).await {
        tracing::warn!("failed to release lease {name} held by {holder}: {e}");
    }
    Ok(Some(token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    /// Minimal subset of migration `022_consistency_kernel` needed here.
    const TEST_SCHEMA: &str = "
        CREATE TABLE IF NOT EXISTS job_leases (
            name        TEXT PRIMARY KEY,
            epoch       INTEGER NOT NULL DEFAULT 0,
            holder      TEXT NOT NULL,
            expires_at  INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS outbox (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            topic        TEXT NOT NULL,
            payload      TEXT NOT NULL,
            created_at   INTEGER NOT NULL DEFAULT (unixepoch()),
            published_at INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_outbox_unpublished
            ON outbox (published_at) WHERE published_at IS NULL;
    ";

    async fn memory_db() -> Arc<libsql::Database> {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .expect("build in-memory database");
        Arc::new(db)
    }

    /// Consistency over a `:memory:` database with the schema applied.
    ///
    /// The schema is applied on the *same* connection the kernel lazily
    /// caches, because every `connect()` on an in-memory database opens a
    /// fresh, empty database.
    async fn consistency_with_schema() -> Consistency {
        let consistency = Consistency::new(memory_db().await);
        consistency
            .connection()
            .await
            .expect("connect for schema setup")
            .execute_batch(TEST_SCHEMA)
            .await
            .expect("apply test schema");
        consistency
    }

    #[tokio::test]
    async fn lease_table_is_created_defensively_without_migration() {
        // No schema setup at all: the kernel must self-initialize job_leases
        // (migration 022 not applied) instead of failing the first claim.
        let consistency = Consistency::new(memory_db().await);
        let token = consistency
            .claim_lease("job.bootstrap", "holder-1", Duration::from_secs(60))
            .await
            .expect("claim on uninitialized database")
            .expect("first claim");
        assert_eq!(token.epoch, 1);
    }

    /// Force the named lease to be expired according to the DB clock.
    async fn force_expire(consistency: &Consistency, name: &str) {
        consistency
            .connection()
            .await
            .expect("conn")
            .execute(
                "UPDATE job_leases SET expires_at = unixepoch() - 1 WHERE name = ?1",
                libsql::params![name],
            )
            .await
            .expect("force expire");
    }

    #[tokio::test]
    async fn claim_lease_first_claim_grants_epoch_one() {
        let consistency = consistency_with_schema().await;
        let token = consistency
            .claim_lease("job.a", "holder-1", Duration::from_secs(60))
            .await
            .expect("claim");
        let token = token.expect("first claim must succeed");
        assert_eq!(token.name, "job.a");
        assert_eq!(token.epoch, 1);
        assert_eq!(token.holder, "holder-1");
    }

    #[tokio::test]
    async fn claim_lease_second_holder_blocked_while_live() {
        let consistency = consistency_with_schema().await;
        consistency
            .claim_lease("job.a", "holder-1", Duration::from_secs(60))
            .await
            .expect("claim")
            .expect("first claim");

        let blocked = consistency
            .claim_lease("job.a", "holder-2", Duration::from_secs(60))
            .await
            .expect("claim attempt");
        assert!(blocked.is_none(), "live lease must block other holders");
    }

    #[tokio::test]
    async fn claim_lease_after_expiry_increments_epoch() {
        let consistency = consistency_with_schema().await;
        let first = consistency
            .claim_lease("job.a", "holder-1", Duration::from_secs(60))
            .await
            .expect("claim")
            .expect("first claim");
        force_expire(&consistency, "job.a").await;

        let second = consistency
            .claim_lease("job.a", "holder-2", Duration::from_secs(60))
            .await
            .expect("claim after expiry")
            .expect("expired lease must be claimable");
        assert_eq!(second.epoch, first.epoch + 1);
        assert_eq!(second.holder, "holder-2");
    }

    #[tokio::test]
    async fn fencing_validates_for_holder_and_invalidates_after_reclaim() {
        let consistency = consistency_with_schema().await;
        let stale = consistency
            .claim_lease("job.a", "holder-1", Duration::from_secs(60))
            .await
            .expect("claim")
            .expect("first claim");
        assert!(consistency
            .validate_fencing(&stale)
            .await
            .expect("validate"));

        force_expire(&consistency, "job.a").await;
        let current = consistency
            .claim_lease("job.a", "holder-2", Duration::from_secs(60))
            .await
            .expect("claim")
            .expect("reclaim");

        assert!(
            !consistency
                .validate_fencing(&stale)
                .await
                .expect("validate stale"),
            "stale epoch must fail fencing after a new claim"
        );
        assert!(
            consistency
                .validate_fencing(&current)
                .await
                .expect("validate current"),
            "current epoch must pass fencing"
        );
    }

    #[tokio::test]
    async fn release_lease_allows_immediate_reclaim() {
        let consistency = consistency_with_schema().await;
        let first = consistency
            .claim_lease("job.a", "holder-1", Duration::from_secs(60))
            .await
            .expect("claim")
            .expect("first claim");

        consistency
            .release_lease("job.a", "holder-1")
            .await
            .expect("release");

        let second = consistency
            .claim_lease("job.a", "holder-2", Duration::from_secs(60))
            .await
            .expect("claim after release")
            .expect("released lease must be claimable");
        assert_eq!(second.epoch, first.epoch + 1);

        // Releasing a lease held by someone else is a no-op, not an error.
        consistency
            .release_lease("job.a", "holder-1")
            .await
            .expect("non-holder release is a no-op");
    }

    #[tokio::test]
    async fn enqueue_outbox_commits_row_with_caller_transaction() {
        let consistency = consistency_with_schema().await;
        let conn = consistency.connection().await.expect("conn").clone();

        let tx = conn.transaction().await.expect("begin tx");
        let id = enqueue_outbox(&tx, "decision.finalized", r#"{"id":1}"#)
            .await
            .expect("enqueue");
        assert_eq!(id, 1);
        tx.commit().await.expect("commit");

        let mut rows = conn
            .query(
                "SELECT topic, payload, published_at FROM outbox WHERE id = 1",
                (),
            )
            .await
            .expect("select");
        let row = rows.next().await.expect("row").expect("row exists");
        let topic: String = row.get(0).expect("topic");
        let payload: String = row.get(1).expect("payload");
        let published_at: Option<i64> = row.get(2).expect("published_at");
        assert_eq!(topic, "decision.finalized");
        assert_eq!(payload, r#"{"id":1}"#);
        assert!(published_at.is_none());
    }

    #[tokio::test]
    async fn enqueue_outbox_rolls_back_with_caller_transaction() {
        let consistency = consistency_with_schema().await;
        let conn = consistency.connection().await.expect("conn").clone();

        let tx = conn.transaction().await.expect("begin tx");
        enqueue_outbox(&tx, "doomed.event", "{}")
            .await
            .expect("enqueue");
        tx.rollback().await.expect("rollback");

        let mut rows = conn
            .query("SELECT COUNT(*) AS n FROM outbox", ())
            .await
            .expect("select");
        let row = rows.next().await.expect("row").expect("row exists");
        let n: i64 = row.get(0).expect("count");
        assert_eq!(n, 0, "rolled-back event must not be durable");
    }

    struct MockPublisher {
        published: Mutex<Vec<(String, String)>>,
        calls: AtomicUsize,
        failures_remaining: AtomicUsize,
    }

    impl MockPublisher {
        fn new(failures: usize) -> Self {
            Self {
                published: Mutex::new(Vec::new()),
                calls: AtomicUsize::new(0),
                failures_remaining: AtomicUsize::new(failures),
            }
        }

        fn published_count(&self) -> usize {
            self.published.lock().expect("lock").len()
        }
    }

    #[async_trait::async_trait]
    impl OutboxPublisher for MockPublisher {
        async fn publish(&self, topic: &str, payload: &str) -> Result<(), Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.failures_remaining.load(Ordering::SeqCst) > 0 {
                self.failures_remaining.fetch_sub(1, Ordering::SeqCst);
                return Err(Error::Publish {
                    topic: topic.to_string(),
                    detail: "injected failure".to_string(),
                });
            }
            self.published
                .lock()
                .expect("lock")
                .push((topic.to_string(), payload.to_string()));
            Ok(())
        }
    }

    /// File-backed database for relay tests: `run_outbox_relay` opens its own
    /// connection from the `Database`, and every connection to a `:memory:`
    /// database is a fresh, empty database — so the relay must run against a
    /// real (temp-file) database to share state with the test.
    async fn file_db() -> (Arc<libsql::Database>, libsql::Connection) {
        let (db, conn) = file_db_bare().await;
        conn.execute_batch(TEST_SCHEMA)
            .await
            .expect("apply test schema");
        (db, conn)
    }

    /// Temp-file database with no schema applied (defensive-bootstrap tests).
    async fn file_db_bare() -> (Arc<libsql::Database>, libsql::Connection) {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "freshcredit_outbox_relay_test_{}_{}.db",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_file(&path);
        let db = libsql::Builder::new_local(path.to_string_lossy().as_ref())
            .build()
            .await
            .expect("build file database");
        let db = Arc::new(db);
        let conn = db.connect().expect("connect");
        (db, conn)
    }

    async fn insert_outbox_row(conn: &libsql::Connection, topic: &str, payload: &str) {
        conn.execute(
            "INSERT INTO outbox (topic, payload) VALUES (?1, ?2)",
            libsql::params![topic, payload],
        )
        .await
        .expect("insert outbox row");
    }

    async fn published_at_stamped(conn: &libsql::Connection, id: i64) -> bool {
        let mut rows = conn
            .query(
                "SELECT published_at IS NOT NULL FROM outbox WHERE id = ?1",
                libsql::params![id],
            )
            .await
            .expect("select");
        let row = rows.next().await.expect("row").expect("row exists");
        row.get::<i32>(0).expect("stamped flag") != 0
    }

    async fn wait_until<F: Fn() -> bool + Send + Sync>(what: &str, condition: F) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !condition() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
    }

    #[tokio::test]
    async fn relay_delivers_and_stamps_published_rows() {
        let (db, conn) = file_db().await;
        insert_outbox_row(&conn, "topic.one", r#"{"n":1}"#).await;
        insert_outbox_row(&conn, "topic.two", r#"{"n":2}"#).await;

        let mock = Arc::new(MockPublisher::new(0));
        let publisher: Arc<dyn OutboxPublisher> = mock.clone();
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let relay = tokio::spawn(run_outbox_relay(
            db,
            publisher,
            Duration::from_millis(20),
            16,
            shutdown_rx,
        ));

        wait_until("both events delivered", || mock.published_count() == 2).await;
        shutdown_tx.send(true).expect("signal shutdown");
        relay.await.expect("relay join").expect("relay exit");

        let mut rows = conn
            .query("SELECT id FROM outbox ORDER BY id", ())
            .await
            .expect("select");
        let mut ids = Vec::new();
        while let Some(row) = rows.next().await.expect("row") {
            ids.push(row.get::<i64>(0).expect("id"));
        }
        assert_eq!(ids.len(), 2);
        for id in ids {
            assert!(
                published_at_stamped(&conn, id).await,
                "row {id} must be stamped after delivery"
            );
        }
    }

    #[tokio::test]
    async fn relay_retries_row_after_publish_failure() {
        let (db, conn) = file_db().await;
        insert_outbox_row(&conn, "flaky.topic", "{}").await;

        let mock = Arc::new(MockPublisher::new(1));
        let publisher: Arc<dyn OutboxPublisher> = mock.clone();
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let relay = tokio::spawn(run_outbox_relay(
            db,
            publisher,
            Duration::from_millis(20),
            16,
            shutdown_rx,
        ));

        wait_until("event delivered after retry", || {
            mock.published_count() == 1
        })
        .await;
        shutdown_tx.send(true).expect("signal shutdown");
        relay.await.expect("relay join").expect("relay exit");

        assert!(
            mock.calls.load(Ordering::SeqCst) >= 2,
            "failing publish must be retried on a later poll"
        );
        assert!(published_at_stamped(&conn, 1).await);
    }

    #[tokio::test]
    async fn relay_creates_outbox_table_defensively_without_migration() {
        // No schema setup at all: the relay must create the outbox table (+
        // partial index) before its first poll instead of erroring out.
        let (db, conn) = file_db_bare().await;
        let mock = Arc::new(MockPublisher::new(0));
        let publisher: Arc<dyn OutboxPublisher> = mock.clone();
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let relay = tokio::spawn(run_outbox_relay(
            db,
            publisher,
            Duration::from_millis(20),
            16,
            shutdown_rx,
        ));

        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let mut rows = conn
                    .query(
                        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'outbox'",
                        (),
                    )
                    .await
                    .expect("probe sqlite_master");
                if rows.next().await.expect("rows").is_some() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("relay must create the outbox table");

        insert_outbox_row(&conn, "bootstrap.topic", "{}").await;
        wait_until("event delivered", || mock.published_count() == 1).await;
        shutdown_tx.send(true).expect("signal shutdown");
        relay.await.expect("relay join").expect("relay exit");
        assert!(published_at_stamped(&conn, 1).await);
    }

    #[tokio::test]
    async fn relay_stops_promptly_on_shutdown() {
        let (db, _conn) = file_db().await;
        let publisher: Arc<dyn OutboxPublisher> = Arc::new(MockPublisher::new(0));
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let relay = tokio::spawn(run_outbox_relay(
            db,
            publisher,
            Duration::from_secs(3600),
            16,
            shutdown_rx,
        ));

        tokio::time::sleep(Duration::from_millis(100)).await;
        shutdown_tx.send(true).expect("signal shutdown");

        tokio::time::timeout(Duration::from_secs(2), relay)
            .await
            .expect("relay must stop promptly on shutdown")
            .expect("relay join")
            .expect("relay exit");
    }

    #[tokio::test]
    async fn run_with_lease_skips_job_when_lease_held_elsewhere() {
        let consistency = consistency_with_schema().await;
        let taken = consistency
            .claim_lease("job.a", "other-holder", Duration::from_secs(60))
            .await
            .expect("claim")
            .expect("claim");

        let ran = Arc::new(AtomicUsize::new(0));
        let ran_in_job = Arc::clone(&ran);
        let result = run_with_lease(
            &consistency,
            "job.a",
            "our-holder",
            Duration::from_secs(60),
            move |_token| {
                let ran_in_job = Arc::clone(&ran_in_job);
                async move {
                    ran_in_job.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .await
        .expect("run_with_lease");
        assert!(result.is_none(), "job must not run while lease is taken");
        assert_eq!(ran.load(Ordering::SeqCst), 0);
        drop(taken);
    }

    #[tokio::test]
    async fn run_with_lease_runs_job_and_releases() {
        let consistency = consistency_with_schema().await;

        let ran = Arc::new(AtomicUsize::new(0));
        let validated = Arc::new(AtomicUsize::new(0));
        let ran_in_job = Arc::clone(&ran);
        let consistency_in_job = consistency.clone();
        let validated_in_job = Arc::clone(&validated);
        let result = run_with_lease(
            &consistency,
            "job.a",
            "holder-1",
            Duration::from_secs(60),
            move |token| {
                let ran_in_job = Arc::clone(&ran_in_job);
                let consistency_in_job = consistency_in_job.clone();
                let validated_in_job = Arc::clone(&validated_in_job);
                async move {
                    ran_in_job.fetch_add(1, Ordering::SeqCst);
                    if consistency_in_job.validate_fencing(&token).await? {
                        validated_in_job.fetch_add(1, Ordering::SeqCst);
                    }
                    Ok(())
                }
            },
        )
        .await
        .expect("run_with_lease");

        let token = result.expect("job ran under lease");
        assert_eq!(token.epoch, 1);
        assert_eq!(ran.load(Ordering::SeqCst), 1);
        assert_eq!(validated.load(Ordering::SeqCst), 1);

        // Lease was released: another holder claims immediately, and the old
        // token no longer fences.
        let next = consistency
            .claim_lease("job.a", "holder-2", Duration::from_secs(60))
            .await
            .expect("claim after release")
            .expect("released lease must be claimable");
        assert_eq!(next.epoch, 2);
        assert!(!consistency
            .validate_fencing(&token)
            .await
            .expect("validate released token"));
    }
}
