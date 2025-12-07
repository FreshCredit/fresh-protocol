//! Security utilities for FreshCredit

pub mod clock;

use blake2::{Blake2b512, Digest};
use anyhow::Result;

/// Generate Blake2b-256 hash from serializable data
pub fn blake2_256_hex<T: serde::Serialize>(value: &T) -> Result<String> {
    // Serialize to canonical JSON bytes
    let json_bytes = serde_json::to_vec(value)
        .map_err(|e| anyhow::anyhow!("Failed to serialize value to JSON: {e}"))?;

    // Compute Blake2b-512 hash (then truncate to 256 bits for compatibility)
    let mut hasher = Blake2b512::new();
    hasher.update(&json_bytes);
    let result = hasher.finalize();

    // Return lowercase hex representation (first 32 bytes = 256 bits)
    Ok(hex::encode(&result[..32]))
}

/// Validate data integrity using hash
pub fn verify_hash<T: serde::Serialize>(value: &T, expected_hash: &str) -> Result<bool> {
    let computed_hash = blake2_256_hex(value)?;
    Ok(computed_hash == expected_hash)
}
