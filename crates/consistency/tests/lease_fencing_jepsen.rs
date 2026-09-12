//! Jepsen-style fencing tests for the Wave 2 consistency kernel.
//!
//! These tests treat `freshcredit_consistency` as a black box and check the
//! two safety properties the kernel exists to provide:
//!
//! 1. **Fencing invariant** — at no instant may two different holders hold a
//!    valid token for the same lease name. A stale holder whose lease was
//!    taken over must never observe `validate_fencing == true` afterwards.
//! 2. **Epoch monotonicity** — fencing epochs are persisted and strictly
//!    increase on every successful claim, including across holder changes.
//!
//! The platform database is a real (file-backed) local libsql database so the
//! tests exercise the same storage path as production; a fresh database file
//! is created per test.
//!
//! Note: lease expiry is tracked with the database's `unixepoch()` (second
//! resolution), so the shortest meaningful TTL is one second; anything
//! shorter truncates to zero and expires immediately.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use freshcredit_consistency::{Consistency, FencingToken};
use tokio::time::{sleep, timeout};

/// Full Wave 2 schema (durable idempotency, job leases, outbox).
const SCHEMA_SQL: &str = include_str!("migrations/022_consistency_kernel.up.sql");

const OPERATION_TIMEOUT: Duration = Duration::from_secs(10);
/// Hard deadline for the holder battle; the battle normally ends as soon as
/// both holders have won the lease at least once.
const BATTLE_DEADLINE: Duration = Duration::from_secs(30);
/// Grace beyond a token's TTL during which a true validation sample is still
/// acceptable (scheduler jitter on a loaded machine).
const EXPIRY_GRACE: Duration = Duration::from_millis(100);
/// One second: the lease clock has second resolution (see file docs).
const LEASE_TTL: Duration = Duration::from_secs(1);
/// Sequential claim/release cycles in the epoch monotonicity test.
const CYCLES: usize = 50;

#[derive(Debug, Clone)]
struct TrueSample {
    /// Global sequence number: samples are totally ordered by this.
    seq: u64,
    holder: String,
    epoch: i64,
    /// When the validated token was originally claimed.
    acquired_at: Instant,
    /// When the validation returned true.
    validated_at: Instant,
}

#[derive(Debug, Clone)]
struct ClaimRecord {
    holder: String,
    epoch: i64,
    acquired_at: Instant,
}

struct Shared {
    /// Latest token each holder believes it holds.
    slots: [Mutex<Option<FencingToken>>; 2],
    /// All successful claims, in observation order.
    claims: Mutex<Vec<ClaimRecord>>,
    /// All validation samples that returned true, in observation order.
    true_samples: Mutex<Vec<TrueSample>>,
    /// Rounds (validator iterations) where both holders validated true.
    /// Pair is (`epoch_holder_a`, `epoch_holder_b`); A is always sampled
    /// first.
    both_valid_rounds: Mutex<Vec<(i64, i64)>>,
    /// Validator rounds in which at least one token was validated.
    rounds_seen: AtomicUsize,
    /// Successful claims per holder slot.
    holder_claims: [AtomicUsize; 2],
}

impl Shared {
    const fn new() -> Self {
        Self {
            slots: [Mutex::new(None), Mutex::new(None)],
            claims: Mutex::new(Vec::new()),
            true_samples: Mutex::new(Vec::new()),
            both_valid_rounds: Mutex::new(Vec::new()),
            rounds_seen: AtomicUsize::new(0),
            holder_claims: [AtomicUsize::new(0), AtomicUsize::new(0)],
        }
    }

    fn both_holders_won(&self) -> bool {
        self.holder_claims[0].load(Ordering::Relaxed) > 0
            && self.holder_claims[1].load(Ordering::Relaxed) > 0
    }
}

/// One operation guard: a stuck claim/validation fails the test instead of
/// hanging forever.
async fn guarded<Fut, T>(op: Fut) -> T
where
    Fut: std::future::Future<Output = Result<T, freshcredit_consistency::Error>>,
{
    match timeout(OPERATION_TIMEOUT, op).await {
        Ok(Ok(value)) => value,
        Ok(Err(err)) => panic!("consistency store error: {err}"),
        Err(elapsed) => {
            panic!("consistency operation timed out after {OPERATION_TIMEOUT:?}: {elapsed}")
        }
    }
}

/// Claim with a small retry loop to tolerate transient `SQLite` busy errors
/// under contention; panics if the lease never becomes claimable.
async fn claim_with_retry(
    consistency: &Consistency,
    name: &str,
    holder: &str,
    ttl: Duration,
) -> Option<FencingToken> {
    let mut last_err = None;
    for _ in 0..5 {
        match timeout(
            OPERATION_TIMEOUT,
            consistency.claim_lease(name, holder, ttl),
        )
        .await
        {
            Ok(Ok(result)) => return result,
            Ok(Err(err)) => {
                last_err = Some(err.to_string());
                sleep(Duration::from_millis(10)).await;
            }
            Err(elapsed) => panic!("claim_lease timed out after {OPERATION_TIMEOUT:?}: {elapsed}"),
        }
    }
    panic!("claim_lease kept failing under retry: {last_err:?}");
}

async fn platform_db(test_name: &str) -> (Arc<libsql::Database>, std::path::PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "fc-consistency-jepsen-{test_name}-{}-{nanos}.db",
        std::process::id()
    ));
    let db = libsql::Builder::new_local(&path)
        .build()
        .await
        .expect("build platform db");
    let conn = db.connect().expect("connect to platform db");
    conn.execute_batch(SCHEMA_SQL)
        .await
        .expect("apply 022_consistency_kernel schema");
    (Arc::new(db), path)
}

fn cleanup_db(path: &std::path::Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let p = std::path::PathBuf::from(format!("{}{suffix}", path.display()));
        let _ = std::fs::remove_file(p);
    }
}

/// Holder loop: battles for the lease until `stop` is set (set by the
/// supervisor as soon as BOTH holders have won at least once, or when the
/// battle deadline hits). Does NOT release; lets the TTL expire so a
/// competing holder can take over (the dangerous window the fencing token
/// exists for).
async fn holder_loop(
    consistency: Arc<Consistency>,
    holder: &'static str,
    slot_idx: usize,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
) {
    let mut own_claims = 0u32;
    while !stop.load(Ordering::Relaxed) {
        match claim_with_retry(
            consistency.as_ref(),
            "jepsen.fencing.battle",
            holder,
            LEASE_TTL,
        )
        .await
        {
            Some(token) => {
                own_claims += 1;
                shared.holder_claims[slot_idx].fetch_add(1, Ordering::Relaxed);
                let acquired_at = Instant::now();
                *shared.slots[slot_idx].lock().expect("slot lock") = Some(token.clone());
                shared
                    .claims
                    .lock()
                    .expect("claims lock")
                    .push(ClaimRecord {
                        holder: holder.to_string(),
                        epoch: token.epoch,
                        acquired_at,
                    });
                // Hold briefly to widen the overlap window where a stale
                // holder could still try to validate.
                sleep(Duration::from_millis(60)).await;
            }
            None => {
                // Someone else holds it; try again quickly.
                sleep(Duration::from_millis(10)).await;
            }
        }
    }
    assert!(
        own_claims > 0,
        "holder {holder} never won the lease — test produced no contention"
    );
}

/// Validator loop: continuously validates whatever token each holder
/// currently believes it holds, recording every true sample with a total
/// order. A and B are validated back-to-back each round.
async fn validator_loop(consistency: Arc<Consistency>, shared: Arc<Shared>, stop: Arc<AtomicBool>) {
    let mut seq = 0u64;
    while !stop.load(Ordering::Relaxed) {
        let mut round = [None, None];
        for (idx, slot) in shared.slots.iter().enumerate() {
            let token = slot.lock().expect("slot lock").clone();
            if let Some(token) = token {
                let valid = guarded(consistency.validate_fencing(&token)).await;
                if valid {
                    let acquired_at = {
                        let claims = shared.claims.lock().expect("claims lock");
                        claims
                            .iter()
                            .find(|c| c.epoch == token.epoch)
                            .map_or_else(Instant::now, |c| c.acquired_at)
                    };
                    seq += 1;
                    shared
                        .true_samples
                        .lock()
                        .expect("true samples lock")
                        .push(TrueSample {
                            seq,
                            holder: token.holder.clone(),
                            epoch: token.epoch,
                            acquired_at,
                            validated_at: Instant::now(),
                        });
                }
                round[idx] = Some(valid);
            }
        }
        if round.iter().any(Option::is_some) {
            shared.rounds_seen.fetch_add(1, Ordering::Relaxed);
            if let [Some(a_valid), Some(b_valid)] = round {
                if a_valid && b_valid {
                    let epoch_a = shared.slots[0]
                        .lock()
                        .expect("slot lock")
                        .as_ref()
                        .unwrap()
                        .epoch;
                    let epoch_b = shared.slots[1]
                        .lock()
                        .expect("slot lock")
                        .as_ref()
                        .unwrap()
                        .epoch;
                    shared
                        .both_valid_rounds
                        .lock()
                        .expect("both valid lock")
                        .push((epoch_a, epoch_b));
                }
            }
        }
        sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_holders_never_both_hold_valid_token() {
    let (db, path) = platform_db("battle").await;
    // One shared handle: every task goes through the same cached connection,
    // so physical SQLite write serialization cannot be mistaken for a
    // fencing violation (or mask one).
    let consistency = Arc::new(Consistency::new(db));
    let shared = Arc::new(Shared::new());
    let stop = Arc::new(AtomicBool::new(false));

    let handles = [
        tokio::spawn(holder_loop(
            Arc::clone(&consistency),
            "holder-a",
            0,
            shared.clone(),
            stop.clone(),
        )),
        tokio::spawn(holder_loop(
            Arc::clone(&consistency),
            "holder-b",
            1,
            shared.clone(),
            stop.clone(),
        )),
        tokio::spawn(validator_loop(
            Arc::clone(&consistency),
            shared.clone(),
            stop.clone(),
        )),
    ];

    // Supervise: end the battle as soon as both holders have won at least
    // once (plus a short tail so the validator samples the final state), or
    // fail at the deadline.
    let deadline = Instant::now() + BATTLE_DEADLINE;
    while !shared.both_holders_won() {
        assert!(
            Instant::now() < deadline,
            "battle deadline hit before both holders won the lease"
        );
        sleep(Duration::from_millis(50)).await;
    }
    sleep(Duration::from_millis(200)).await;
    stop.store(true, Ordering::Relaxed);
    for handle in handles {
        handle.await.expect("task join");
    }

    check_fencing_model(&shared);
    cleanup_db(&path);
}

/// Post-run model check over everything the validator observed.
fn check_fencing_model(shared: &Shared) {
    let claims = shared.claims.lock().expect("claims lock").clone();
    let true_samples = shared.true_samples.lock().expect("samples lock").clone();
    let both_valid_rounds = shared
        .both_valid_rounds
        .lock()
        .expect("both valid lock")
        .clone();

    // Liveness: real contention happened.
    assert!(
        claims.len() >= 2,
        "expected multiple claims, got {}",
        claims.len()
    );
    let holders: HashSet<&str> = claims.iter().map(|c| c.holder.as_str()).collect();
    assert_eq!(
        holders.len(),
        2,
        "both holders must have won the lease at least once"
    );

    // (a) Every successful claim got a distinct epoch: total successful
    //     claims == number of distinct epochs observed.
    let distinct_epochs: HashSet<i64> = claims.iter().map(|c| c.epoch).collect();
    assert_eq!(
        distinct_epochs.len(),
        claims.len(),
        "claims must never share an epoch: {claims:?}"
    );
    // Every validated-true epoch must correspond to a real claim.
    for sample in &true_samples {
        assert!(
            distinct_epochs.contains(&sample.epoch),
            "validated epoch {} was never claimed",
            sample.epoch
        );
    }

    // (b) Fencing invariant (temporal form): ordered by observation time,
    //     epochs of true samples never decrease — a stale epoch must never
    //     validate true after a newer epoch validated true.
    let mut ordered = true_samples.clone();
    ordered.sort_by_key(|s| s.seq);
    for window in ordered.windows(2) {
        let (earlier, later) = (&window[0], &window[1]);
        assert!(
            later.epoch >= earlier.epoch,
            "fencing violation: epoch {} ({} valid at {:?}) resurrected after epoch {} ({} valid at {:?})",
            earlier.epoch,
            earlier.holder,
            earlier.validated_at,
            later.epoch,
            later.holder,
            later.validated_at
        );
    }

    // (c) Paired form: in rounds where both holders validated true (A sampled
    //     before B), the earlier-sampled token must carry the strictly lower
    //     epoch — i.e. the handover was observed in order, never reversed.
    for (epoch_a, epoch_b) in &both_valid_rounds {
        assert!(
            epoch_a < epoch_b,
            "fencing violation: A's epoch {epoch_a} still valid in the same round as newer B epoch {epoch_b}"
        );
    }

    // (d) No expired token may ever validate true: a true sample observed
    //     after token acquisition + TTL + grace means stale commits would be
    //     possible in production.
    for sample in &true_samples {
        let deadline = sample.acquired_at + LEASE_TTL + EXPIRY_GRACE;
        assert!(
            sample.validated_at <= deadline,
            "expired lease validated true: {} epoch {} validated at {:?} (acquired {:?}, ttl {LEASE_TTL:?})",
            sample.holder,
            sample.epoch,
            sample.validated_at,
            sample.acquired_at
        );
    }

    // (e) Sanity: the validator actually observed tokens.
    assert!(
        shared.rounds_seen.load(Ordering::Relaxed) > 0,
        "validator never saw any token"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn epochs_never_decrease_across_claim_cycles() {
    let (db, path) = platform_db("epochs").await;
    let consistency = Consistency::new(db);

    let mut last_epoch = 0i64;
    for cycle in 0..CYCLES {
        let holder = if cycle % 2 == 0 {
            "holder-a"
        } else {
            "holder-b"
        };
        // Alternate holders so epoch advancement cannot come from an
        // in-memory counter; it must be persisted in the platform DB.
        let token = claim_with_retry(&consistency, "jepsen.epoch.monotonic", holder, LEASE_TTL)
            .await
            .unwrap_or_else(|| panic!("cycle {cycle}: lease should be free after release"));

        assert!(
            token.epoch > last_epoch,
            "epoch decreased: {last_epoch} -> {} at cycle {cycle}",
            token.epoch
        );
        last_epoch = token.epoch;

        // Freshly claimed token must validate true for the claiming holder...
        let valid = guarded(consistency.validate_fencing(&token)).await;
        assert!(valid, "freshly claimed token must validate true");

        guarded(consistency.release_lease("jepsen.epoch.monotonic", holder)).await;

        // ...and must be invalid immediately after release.
        let valid_after_release = guarded(consistency.validate_fencing(&token)).await;
        assert!(
            !valid_after_release,
            "released token must not validate true (cycle {cycle})"
        );
    }

    cleanup_db(&path);
}
