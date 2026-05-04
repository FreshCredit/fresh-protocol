//! Security utilities for FreshCredit
//!
//! Provides cryptographic utilities:
//! - Blake2b hashing for data integrity
//! - AES-256-GCM encryption for sensitive tokens
//! - Time-based security utilities

#![deny(unsafe_code)]
//!
//! # Token Encryption
//!
//! The encryption module provides AES-256-GCM encryption for OAuth tokens.
//!
//! ## Configuration
//! - `FRESHCREDIT_ENCRYPTION_ENABLED`: Set to "false" to disable (testing only)
//! - `FRESHCREDIT_TOKEN_ENCRYPTION_KEY`: 64-character hex key
//!
//! ## Usage
//! ```rust,ignore
//! use freshcredit_security::{encrypt_token, decrypt_token};
//!
//! // Encrypt a token before storage
//! let encrypted = encrypt_token("my-access-token");
//!
//! // Decrypt when retrieving
//! let decrypted = decrypt_token(&encrypted);
//! ```

// Clock re-exported from freshcredit-core-timing to avoid duplication
// Use: freshcredit_security::Clock, freshcredit_security::MockClock, freshcredit_security::SystemClock
pub use freshcredit_core_timing::{Clock, MockClock, SystemClock};

// Backward compatibility: freshcredit_security::clock::Clock still works
pub mod clock {
    //! Clock module - re-exported from freshcredit-core-timing
    pub use freshcredit_core_timing::{Clock, MockClock, SystemClock};
}

pub mod encryption;

// Re-export commonly used items
pub use encryption::{
    decrypt_token, encrypt_token, generate_base64_key, generate_hex_key, get_encryption_config,
    EncryptionConfig, EncryptionError, TokenEncryptor, KEY_SIZE,
};

use anyhow::Result;
use blake2::{Blake2b512, Digest};

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn test_blake2_256_hex_produces_64_char_hash() {
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };
        let hash = blake2_256_hex(&data).unwrap();
        assert_eq!(hash.len(), 64); // 256 bits = 32 bytes = 64 hex chars
    }

    #[test]
    fn test_blake2_256_hex_deterministic() {
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };
        let hash1 = blake2_256_hex(&data).unwrap();
        let hash2 = blake2_256_hex(&data).unwrap();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_blake2_256_hex_different_data_different_hash() {
        let data1 = TestData {
            name: "test".to_string(),
            value: 42,
        };
        let data2 = TestData {
            name: "test".to_string(),
            value: 43,
        };
        let hash1 = blake2_256_hex(&data1).unwrap();
        let hash2 = blake2_256_hex(&data2).unwrap();
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_verify_hash_valid() {
        let data = TestData {
            name: "verify".to_string(),
            value: 100,
        };
        let hash = blake2_256_hex(&data).unwrap();
        assert!(verify_hash(&data, &hash).unwrap());
    }

    #[test]
    fn test_verify_hash_invalid() {
        let data = TestData {
            name: "verify".to_string(),
            value: 100,
        };
        let wrong_hash = "0".repeat(64);
        assert!(!verify_hash(&data, &wrong_hash).unwrap());
    }

    #[test]
    fn test_blake2_256_hex_lowercase() {
        let data = TestData {
            name: "test".to_string(),
            value: 1,
        };
        let hash = blake2_256_hex(&data).unwrap();
        assert_eq!(hash, hash.to_lowercase());
    }
}
