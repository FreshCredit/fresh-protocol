//! Helper functions for the blockchain client

use std::time::Duration;

use sha2::{Digest, Sha256};

/// Default cap on how long a caller may wait for GRANDPA finality before the
/// wait is aborted. Overridable via `BLOCKCHAIN_FINALIZE_TIMEOUT_SECS`.
pub(crate) const DEFAULT_FINALIZE_TIMEOUT_SECS: u64 = 180;

/// Timeout for `wait_for_finalized_success`, bounded so a stalled finality
/// listener can never block a caller forever. Read per call from the
/// environment so tests and operators can override without a rebuild.
pub(crate) fn finalize_timeout() -> Duration {
    std::env::var("BLOCKCHAIN_FINALIZE_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map_or(
            Duration::from_secs(DEFAULT_FINALIZE_TIMEOUT_SECS),
            Duration::from_secs,
        )
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Convert `user_id` string to Substrate `AccountId32` using SHA-256
/// This provides deterministic mapping from external user IDs to on-chain accounts
///
/// Note: Currently unused as hashes are stored under service account.
/// Kept for future pallet update where hashes may be stored under user accounts.
#[allow(dead_code)]
pub(crate) fn user_id_to_account_id(user_id: &str) -> subxt::utils::AccountId32 {
    let mut hasher = Sha256::new();
    hasher.update(b"freshcredit-user:");
    hasher.update(user_id.as_bytes());
    let hash = hasher.finalize();
    let bytes: [u8; 32] = hash.into();
    subxt::utils::AccountId32::from(bytes)
}

/// Generate a `report_id` from `user_id` and `report_type`
pub(crate) fn generate_report_id(user_id: &str, report_type: &str, data_hash: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"freshcredit-report:");
    hasher.update(user_id.as_bytes());
    hasher.update(b":");
    hasher.update(report_type.as_bytes());
    hasher.update(b":");
    hasher.update(data_hash.as_bytes());
    hasher.finalize().into()
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Parse a 32-byte hex string (with optional `0x` prefix) into a `[u8; 32]`
pub(crate) fn parse_h256_hex(hash_hex: &str) -> Option<[u8; 32]> {
    let hash_str = hash_hex.trim_start_matches("0x");
    let bytes = hex::decode(hash_str).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Some(arr)
}

/// Extract the last 32 bytes from a storage key as a `report_id`
pub(crate) fn extract_report_id_from_key(key_bytes: &[u8]) -> [u8; 32] {
    if key_bytes.len() >= 32 {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&key_bytes[key_bytes.len() - 32..]);
        arr
    } else {
        [0u8; 32]
    }
}
