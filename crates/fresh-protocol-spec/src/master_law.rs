//! Canonical Master Stability Law kernel matching `formal/lean/TrustStack/MasterLaw.lean`,
//! `formal/zk_master_law_stp.py`, `uniffi_bridge`, and `sim-core::master_law`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Domain errors for canonical Master Law evaluation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MasterLawError {
    /// One or more floating-point inputs were NaN or infinite.
    #[error("non-finite floating-point input")]
    NonFiniteInput,
    /// Recognition gate r_t fell outside [0, 1].
    #[error("recognition gate r_t must lie in [0, 1]")]
    InvalidRecognitionGate,
    /// State or configuration parameter fell outside the verified domain.
    #[error("parameter or state outside verified [0, 1] / positive domain")]
    InvalidParameterDomain,
}

/// Canonical Master Stability Law parameters on `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MasterLawParams {
    /// Positive mismatch learning rate (lambda+).
    pub lambda_plus: f64,
    /// Negative mismatch learning rate (lambda-).
    pub lambda_minus: f64,
    /// Homeostatic baseline leak rate (delta_S in (0, 1]).
    pub leak_delta: f64,
    /// Baseline trust anchor S_base in [0, 1].
    pub s_base: f64,
    /// Domain volatility normalization scale s_d > 0.
    pub scale_s_d: f64,
}

impl Default for MasterLawParams {
    fn default() -> Self {
        Self {
            lambda_plus: 0.228,
            lambda_minus: 0.912,
            leak_delta: 0.010,
            s_base: 0.500,
            scale_s_d: 1.0,
        }
    }
}

impl MasterLawParams {
    /// Validate parameter domains against `TrustStack.MasterLaw` hypotheses.
    /// # Errors
    ///
    /// Returns `MasterLawError` if any parameter is non-finite or out of domain.
    pub fn validate(&self) -> Result<(), MasterLawError> {
        if !(self.lambda_plus.is_finite()
            && self.lambda_minus.is_finite()
            && self.leak_delta.is_finite()
            && self.s_base.is_finite()
            && self.scale_s_d.is_finite())
        {
            return Err(MasterLawError::NonFiniteInput);
        }
        if self.lambda_plus < 0.0
            || self.lambda_minus < 0.0
            || self.leak_delta <= 0.0
            || self.leak_delta > 1.0
            || !(0.0..=1.0).contains(&self.s_base)
            || self.scale_s_d <= 0.0
        {
            return Err(MasterLawError::InvalidParameterDomain);
        }
        Ok(())
    }

    /// Evaluate one clamped Master Stability Law step on `[0, 1]`.
    /// # Errors
    ///
    /// Returns `MasterLawError` if inputs or parameters are invalid/non-finite.
    pub fn step_clamped(
        &self,
        current_s: f64,
        v_t: f64,
        e_t: f64,
        r_t: f64,
    ) -> Result<f64, MasterLawError> {
        self.validate()?;
        if !(current_s.is_finite() && v_t.is_finite() && e_t.is_finite() && r_t.is_finite()) {
            return Err(MasterLawError::NonFiniteInput);
        }
        if !(0.0..=1.0).contains(&r_t) {
            return Err(MasterLawError::InvalidRecognitionGate);
        }
        if !(0.0..=1.0).contains(&current_s) {
            return Err(MasterLawError::InvalidParameterDomain);
        }
        let s_d = self.scale_s_d.max(1e-4);
        let eps = (v_t - e_t) / s_d;
        let pos_term = self.lambda_plus * eps.max(0.0) * r_t;
        let neg_term = self.lambda_minus * eps.min(0.0) * r_t;
        let leak_term = self.leak_delta * (current_s - self.s_base);
        Ok((current_s + pos_term + neg_term - leak_term).clamp(0.0, 1.0))
    }

    /// Fixed-point integer scale matching `formal/zk_master_law_stp.py` (`SCALE = 10^9`).
    pub const FIXED_SCALE: i128 = 1_000_000_000;

    /// Exact fixed-point integer Master Law step matching `zk_master_law_stp.py` bit-for-bit.
    /// Eliminates IEEE-754 floating-point divergence between Rust edge nodes and ZK-STP circuits.
    /// # Errors
    ///
    /// Returns `MasterLawError` if inputs or parameters fall outside `[0, FIXED_SCALE]`.
    pub fn step_fixed_i64(
        &self,
        current_s_fixed: i64,
        v_fixed: i64,
        e_fixed: i64,
        r_fixed: i64,
    ) -> Result<i64, MasterLawError> {
        self.validate()?;
        let s = i128::from(current_s_fixed);
        let v = i128::from(v_fixed);
        let e = i128::from(e_fixed);
        let r = i128::from(r_fixed);
        if !(0..=Self::FIXED_SCALE).contains(&s)
            || !(0..=Self::FIXED_SCALE).contains(&v)
            || !(0..=Self::FIXED_SCALE).contains(&e)
            || !(0..=Self::FIXED_SCALE).contains(&r)
        {
            return Err(MasterLawError::InvalidParameterDomain);
        }
        #[allow(clippy::cast_possible_truncation)]
        let lp = (self.lambda_plus * (Self::FIXED_SCALE as f64)).round() as i128;
        #[allow(clippy::cast_possible_truncation)]
        let ln = (self.lambda_minus * (Self::FIXED_SCALE as f64)).round() as i128;
        #[allow(clippy::cast_possible_truncation)]
        let delta = (self.leak_delta * (Self::FIXED_SCALE as f64)).round() as i128;
        #[allow(clippy::cast_possible_truncation)]
        let s_base = (self.s_base * (Self::FIXED_SCALE as f64)).round() as i128;

        #[allow(clippy::cast_possible_truncation)]
        let s_d_fixed = (self.scale_s_d.max(1e-4) * (Self::FIXED_SCALE as f64))
            .round()
            .max(1.0) as i128;

        let eps = if s_d_fixed == Self::FIXED_SCALE {
            v - e
        } else {
            ((v - e) * Self::FIXED_SCALE).div_euclid(s_d_fixed)
        };
        let (eps_p, eps_n) = if eps >= 0 { (eps, 0) } else { (0, -eps) };
        let num_d = (lp * eps_p - ln * eps_n) * r;
        let q_d = num_d.div_euclid(Self::FIXED_SCALE * Self::FIXED_SCALE);
        let num_l = delta * (s - s_base);
        let q_l = num_l.div_euclid(Self::FIXED_SCALE);
        let unclamped = s + q_d - q_l;
        let clamped = unclamped.clamp(0, Self::FIXED_SCALE);
        i64::try_from(clamped).map_err(|_| MasterLawError::InvalidParameterDomain)
    }

    /// Z3-verified asymmetric BIBO invariant interval `[S_base - lambda-/delta, S_base + lambda+/delta]`.
    #[must_use]
    pub fn asymmetric_invariant_bounds(&self) -> (f64, f64) {
        (
            self.s_base - (self.lambda_minus / self.leak_delta),
            self.s_base + (self.lambda_plus / self.leak_delta),
        )
    }
}

/// Canonical `TFP_AUDIT_V1` serialization payload matching `uniffi_bridge`, normalizing `-0.0` to `+0.0`.
/// # Errors
///
/// Returns `MasterLawError` if `state_s` is non-finite or outside `[0, 1]`.
pub fn format_audit_entry_payload(
    version: u64,
    prev_hash: &str,
    state_s: f64,
) -> Result<String, MasterLawError> {
    if !state_s.is_finite() || !(0.0..=1.0).contains(&state_s) {
        return Err(MasterLawError::InvalidParameterDomain);
    }
    let canon_s = if state_s == 0.0 { 0.0 } else { state_s };
    Ok(format!("TFP_AUDIT_V1:{version}:{prev_hash}:{canon_s:.9}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_master_law_golden_vectors_match_lean_and_zk() {
        let p = MasterLawParams::default();
        let s1 = p.step_clamped(0.500, 0.850, 0.500, 1.0).unwrap();
        assert!((s1 - 0.579800).abs() < 1e-9, "got {s1}");

        let s2 = p.step_clamped(0.5798, 0.100, 0.750, 1.0).unwrap();
        assert!(s2.abs() < 1e-9, "expected lower clamp 0.0, got {s2}");

        let s3 = p.step_clamped(0.430802, 0.500, 0.500, 1.0).unwrap();
        assert!((s3 - 0.43149398).abs() < 1e-9, "got {s3}");
    }

    #[test]
    fn test_audit_payload_canonicalizes_negative_zero() {
        let a = format_audit_entry_payload(1, &"0".repeat(64), -0.0).unwrap();
        let b = format_audit_entry_payload(1, &"0".repeat(64), 0.0).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_step_fixed_i64_matches_zk_stp_exact_integers() {
        let p = MasterLawParams::default();
        // S_0 = 0.500, V = 0.850, E = 0.500, r = 1.0 -> 579_800_000 (exact)
        let s1 = p
            .step_fixed_i64(500_000_000, 850_000_000, 500_000_000, 1_000_000_000)
            .unwrap();
        assert_eq!(s1, 579_800_000);

        // Severe betrayal hits lower clamp 0
        let s2 = p
            .step_fixed_i64(s1, 100_000_000, 750_000_000, 1_000_000_000)
            .unwrap();
        assert_eq!(s2, 0);

        // Zero mismatch leaks toward S_base = 0.500
        let s3 = p
            .step_fixed_i64(430_802_000, 500_000_000, 500_000_000, 1_000_000_000)
            .unwrap();
        assert_eq!(s3, 431_493_980);
    }

    #[test]
    fn test_step_fixed_i64_and_step_clamped_agree_across_scale_s_d() {
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next_unit = || -> f64 {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 11) as f64) / ((1u64 << 53) as f64)
        };

        for _ in 0..2000 {
            let lp = 0.01 + 0.45 * next_unit();
            let ln = lp + 0.45 * next_unit();
            let delta = 0.01 + 0.25 * next_unit();
            let s_base = next_unit();
            let scale_s_d = 0.5 + 2.5 * next_unit();
            let params = MasterLawParams {
                lambda_plus: lp,
                lambda_minus: ln,
                leak_delta: delta,
                s_base,
                scale_s_d,
            };
            let s = next_unit();
            let v = next_unit();
            let e = next_unit();
            let r = next_unit();

            let s_float = params.step_clamped(s, v, e, r).unwrap();
            let to_fixed = |x: f64| -> i64 { (x * 1_000_000_000.0).round() as i64 };
            let s_fixed = params
                .step_fixed_i64(to_fixed(s), to_fixed(v), to_fixed(e), to_fixed(r))
                .unwrap();
            let diff = (s_float - (s_fixed as f64) / 1_000_000_000.0).abs();
            // Bound within quantization slack across floating vs Euclidean fixed division
            assert!(
                diff <= 5e-8 / delta.max(0.01),
                "scale_s_d={scale_s_d}, diff={diff}, float={s_float}, fixed={s_fixed}"
            );
        }
    }
}
