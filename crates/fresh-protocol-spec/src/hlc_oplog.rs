//! Canonical Hybrid Logical Clock (HLC) + SHA-256 `G-Set` operation-log reducer.
//!
//! Lean 4 theorem `TrustStack.MasterLaw.masterStepClamped_not_comm` proves that
//! clamped `[0, 1]` Master Law updates do **not** commute as a state-based CRDT.
//! Paired with Turso Sync's Last-Push-Wins / rollback-and-replay semantics,
//! multi-device edge clients must converge via an idempotent, content-addressed
//! `G-Set` of validation operations ordered by `(wall_ms, logical, event_id)`.

use crate::master_law::{MasterLawError, MasterLawParams};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Fixed-point nanounit scale matching [`MasterLawParams::FIXED_SCALE`].
pub const FIXED_SCALE: i64 = 1_000_000_000;

/// Hybrid Logical Clock timestamp providing a deterministic total order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HlcTimestamp {
    /// Physical wall-clock component in milliseconds.
    pub wall_ms: u64,
    /// Monotonic logical sub-counter when physical time does not advance.
    pub logical: u32,
}

/// Per-node HLC generator bounded by local monotonic advances.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HlcClock {
    /// Last observed wall-clock component in milliseconds.
    pub wall_ms: u64,
    /// Logical counter at `wall_ms`.
    pub logical: u32,
}

impl HlcClock {
    /// Advance the local clock on an emitted or received event.
    #[must_use]
    pub fn tick(&mut self, physical_ms: u64) -> HlcTimestamp {
        if physical_ms > self.wall_ms {
            self.wall_ms = physical_ms;
            self.logical = 0;
        } else {
            self.logical = self.logical.saturating_add(1);
        }
        HlcTimestamp {
            wall_ms: self.wall_ms,
            logical: self.logical,
        }
    }
}

/// Content-addressed validation event in the fixed-point `G-Set` op-log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationOp {
    /// Hybrid Logical Clock timestamp at origin.
    pub hlc: HlcTimestamp,
    /// Deterministic SHA-256 content digest tying `(actor, seq, v_fixed, e_fixed, r_fixed)`.
    pub event_id: String,
    /// Validation outcome in fixed-point nanounits `[0, FIXED_SCALE]`.
    pub v_fixed: i64,
    /// Expectation prior in fixed-point nanounits `[0, FIXED_SCALE]`.
    pub e_fixed: i64,
    /// Recognition gate in fixed-point nanounits `[0, FIXED_SCALE]`.
    pub r_fixed: i64,
}

impl ValidationOp {
    /// Construct a `ValidationOp` with a canonical SHA-256 `event_id`.
    #[must_use]
    pub fn new(
        hlc: HlcTimestamp,
        actor: &str,
        seq: u64,
        v_fixed: i64,
        e_fixed: i64,
        r_fixed: i64,
    ) -> Self {
        let mut h = Sha256::new();
        h.update(format!("TFP_OP_V1:{actor}:{seq}:{v_fixed}:{e_fixed}:{r_fixed}").as_bytes());
        Self {
            hlc,
            event_id: hex::encode(h.finalize()),
            v_fixed,
            e_fixed,
            r_fixed,
        }
    }
}

/// Deterministically fold an arbitrary-order `G-Set` slice of [`ValidationOp`]s
/// into a final clamped `[0, FIXED_SCALE]` trust standing.
///
/// # Errors
///
/// Returns [`MasterLawError`] if `params`, `s_init_fixed`, or any operation falls
/// outside the verified fixed-point domain.
pub fn replay_gset_fixed_i64(
    params: &MasterLawParams,
    s_init_fixed: i64,
    ops: &[ValidationOp],
) -> Result<i64, MasterLawError> {
    params.validate()?;
    if !(0..=FIXED_SCALE).contains(&s_init_fixed) {
        return Err(MasterLawError::InvalidParameterDomain);
    }

    let mut ordered: BTreeMap<(u64, u32, &str), &ValidationOp> = BTreeMap::new();
    for op in ops {
        ordered
            .entry((op.hlc.wall_ms, op.hlc.logical, op.event_id.as_str()))
            .or_insert(op);
    }

    let mut s = s_init_fixed;
    for op in ordered.into_values() {
        s = params.step_fixed_i64(s, op.v_fixed, op.e_fixed, op.r_fixed)?;
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hlc_gset_converges_identically_under_out_of_order_and_duplicates() {
        let params = MasterLawParams::default();
        let mut clk_a = HlcClock::default();
        let mut clk_b = HlcClock::default();

        let op1 = ValidationOp::new(
            clk_a.tick(1_700_000_000_000),
            "node_A",
            0,
            950_000_000,
            100_000_000,
            FIXED_SCALE,
        );
        let op2 = ValidationOp::new(
            clk_b.tick(1_700_000_000_000),
            "node_B",
            1,
            50_000_000,
            950_000_000,
            FIXED_SCALE,
        );
        let op3 = ValidationOp::new(
            clk_a.tick(1_700_000_000_000),
            "node_A",
            2,
            900_000_000,
            200_000_000,
            FIXED_SCALE,
        );
        let op4 = ValidationOp::new(
            clk_b.tick(1_700_000_010_000),
            "node_B",
            3,
            100_000_000,
            800_000_000,
            FIXED_SCALE,
        );

        let s_fwd = [op1.clone(), op2.clone(), op3.clone(), op4.clone()]
            .iter()
            .fold(500_000_000, |acc, op| {
                params
                    .step_fixed_i64(acc, op.v_fixed, op.e_fixed, op.r_fixed)
                    .unwrap()
            });
        let s_rev = [op4.clone(), op3.clone(), op2.clone(), op1.clone()]
            .iter()
            .fold(500_000_000, |acc, op| {
                params
                    .step_fixed_i64(acc, op.v_fixed, op.e_fixed, op.r_fixed)
                    .unwrap()
            });
        assert_ne!(s_fwd, s_rev, "clamped updates must be non-commutative");

        let stream_a = vec![op1.clone(), op2.clone(), op3.clone(), op4.clone()];
        let stream_b = vec![op4, op2.clone(), op1, op3, op2];
        assert_eq!(
            replay_gset_fixed_i64(&params, 500_000_000, &stream_a).unwrap(),
            replay_gset_fixed_i64(&params, 500_000_000, &stream_b).unwrap()
        );
    }
}
