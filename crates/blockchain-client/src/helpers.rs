//! Helper functions for the blockchain client

use sha2::{Digest, Sha256};

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
