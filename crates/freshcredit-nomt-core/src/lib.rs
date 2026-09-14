// TAG: surface=blockchain owner=blockchain-team rule=BC-001
//! NOMT Core - Unified Merkle Proof Types
//!
//! This crate provides the core types for NOMT (Nearly Optimal Merkle Trie)
//! proofs and verification, shared across all NOMT implementations.

#![warn(missing_docs)]
#![deny(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// A Merkle proof from NOMT storage
///
/// This is the unified `NomtProof` structure that replaces the 4 different
/// implementations found across the codebase:
/// - `crates/nomt-sidecar/src/types.rs` (SOURCE OF TRUTH)
/// - `apps/engine/src/ports/nomt.rs` (incompatible format)
/// - `crates/nomt-wasm/src/lib.rs` (`Vec<u8>` variant)
/// - `crates/blockchain-client/src/types.rs` (opaque blob format)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NomtProof {
    /// Hash of the leaf node (the stored data)
    pub leaf_hash: String,
    /// Sibling hashes along the path to root
    pub siblings: Vec<String>,
    /// Path direction at each level (true = right, false = left)
    pub path: Vec<bool>,
    /// Root hash this proof verifies against
    pub root: String,
}

impl NomtProof {
    /// Create a new NOMT proof
    pub fn new(
        leaf_hash: impl Into<String>,
        siblings: Vec<String>,
        path: Vec<bool>,
        root: impl Into<String>,
    ) -> Self {
        Self {
            leaf_hash: leaf_hash.into(),
            siblings,
            path,
            root: root.into(),
        }
    }

    /// Verify the proof against the expected leaf hash
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    ///
    /// This performs the Merkle proof verification by hashing the leaf
    /// with sibling hashes along the path.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn verify(&self) -> Result<bool, ProofError> {
        if self.siblings.len() != self.path.len() {
            return Err(ProofError::InvalidPath(
                self.siblings.len(),
                self.path.len(),
            ));
        }

        let mut current_hash = hex::decode(&self.leaf_hash)
            .map_err(|_| ProofError::InvalidHash(self.leaf_hash.clone()))?;

        for (i, (sibling, is_right)) in self.siblings.iter().zip(&self.path).enumerate() {
            let sibling_bytes =
                hex::decode(sibling).map_err(|_| ProofError::InvalidHash(sibling.clone()))?;

            // Combine hashes: if we're on the right, sibling is on the left
            let combined = if *is_right {
                // Sibling is left, current is right
                let mut combined = sibling_bytes.clone();
                combined.extend_from_slice(&current_hash);
                combined
            } else {
                // Current is left, sibling is right
                let mut combined = current_hash.clone();
                combined.extend_from_slice(&sibling_bytes);
                combined
            };

            current_hash = blake3::hash(&combined).as_bytes().to_vec();

            tracing::debug!(
                level = i,
                hash = hex::encode(&current_hash),
                "Computed intermediate hash"
            );
        }

        let computed_root = hex::encode(current_hash);
        Ok(computed_root == self.root)
    }

    /// Get the proof depth (number of levels)
    #[must_use]
    pub fn depth(&self) -> usize {
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        self.siblings.len()
    }

    /// Convert to binary format (for WASM/JS compatibility)
    #[must_use]
    pub fn to_binary(&self) -> NomtProofBinary {
        NomtProofBinary {
            leaf_hash: hex::decode(&self.leaf_hash).unwrap_or_default(),
            siblings: self
                .siblings
                .iter()
                .filter_map(|s| hex::decode(s).ok())
                .collect(),
            path: self.path.clone(),
            root: hex::decode(&self.root).unwrap_or_default(),
        }
    }
}

impl fmt::Display for NomtProof {
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NomtProof(leaf={}, depth={}, root={})",
            &self.leaf_hash[..8.min(self.leaf_hash.len())],
            self.depth(),
            &self.root[..8.min(self.root.len())]
        )
    }
}

/// Binary format of `NomtProof` (for WASM/JS compatibility)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NomtProofBinary {
    /// Hash of the leaf node as bytes
    pub leaf_hash: Vec<u8>,
    /// Sibling hashes as byte arrays
    pub siblings: Vec<Vec<u8>>,
    /// Path direction at each level
    pub path: Vec<bool>,
    /// Root hash as bytes
    pub root: Vec<u8>,
}

impl NomtProofBinary {
    /// Convert to string format
    /// # Errors
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    ///
    /// Returns an error if the operation fails.
    pub fn to_string_format(&self) -> Result<NomtProof, ProofError> {
        Ok(NomtProof {
            leaf_hash: hex::encode(&self.leaf_hash),
            siblings: self.siblings.iter().map(hex::encode).collect(),
            path: self.path.clone(),
            root: hex::encode(&self.root),
        })
    }

    /// Verify the proof
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn verify(&self) -> Result<bool, ProofError> {
        self.to_string_format()?.verify()
    }
}

/// Errors that can occur during proof operations
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProofError {
    /// Invalid hash format
    #[error("Invalid hex hash: {0}")]
    InvalidHash(String),
    /// Path and siblings length mismatch
    #[error("Path length ({0}) does not match siblings length ({1})")]
    InvalidPath(usize, usize),
    /// Verification failed
    #[error("Proof verification failed")]
    VerificationFailed,
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Metadata for a stored hash
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HashMetadata {
    /// When the hash was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Original data type (e.g., "report", "audit", "transaction")
    pub data_type: String,
    /// Data identifier
    pub data_id: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
// TAG: surface=blockchain owner=blockchain-team rule=BC-001

impl HashMetadata {
    /// Create new hash metadata
    pub fn new(data_type: impl Into<String>, data_id: impl Into<String>) -> Self {
        Self {
            created_at: chrono::Utc::now(),
            data_type: data_type.into(),
            data_id: data_id.into(),
            description: None,
        }
    }

    /// Add a description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Request to store a hash in NOMT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreRequest {
    /// The data to hash and store
    pub data: String,
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    /// Metadata for the hash
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMetadata>,
}

/// Response from storing a hash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreResponse {
    /// The computed hash
    pub hash: String,
    /// The Merkle root after storage
    pub root: String,
    /// Metadata (echoed back)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMetadata>,
}

/// Request to verify a hash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    /// The hash to verify
    pub hash: String,
    /// Expected root (optional - uses current root if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_root: Option<String>,
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}

/// Response from verifying a hash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResponse {
    /// Whether the verification succeeded
    pub verified: bool,
    /// The proof (if verification succeeded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof: Option<NomtProof>,
    /// Current root hash
    pub current_root: String,
    /// Error message (if verification failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageStats {
    /// Total number of items stored
    pub total_items: u64,
    /// Current Merkle root
    pub current_root: String,
    /// Last updated timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
    /// Storage backend type
    pub backend_type: String,
}

/// Result type for proof operations
pub type ProofResult<T> = Result<T, ProofError>;

/// Compute Blake3 hash of data
pub fn compute_hash(data: impl AsRef<[u8]>) -> String {
    hex::encode(blake3::hash(data.as_ref()).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nomt_proof_creation() {
        let proof = NomtProof::new(
            "abc123",
            vec!["sibling1".to_string(), "sibling2".to_string()],
            vec![true, false],
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            "root123",
        );

        assert_eq!(proof.depth(), 2);
        assert_eq!(proof.leaf_hash, "abc123");
    }

    #[test]
    fn test_nomt_proof_display() {
        let proof = NomtProof::new(
            "abcdef123456",
            vec!["sib1".to_string()],
            vec![true],
            "root789012",
        );

        let display = format!("{proof}");
        assert!(display.contains("NomtProof"));
        assert!(display.contains("depth=1"));
    }

    #[test]
    fn test_proof_verify_path_mismatch() {
        let proof = NomtProof {
            leaf_hash: "abc".to_string(),
            siblings: vec!["s1".to_string(), "s2".to_string()],
            path: vec![true], // Mismatched length
            root: "root".to_string(),
        };

        let result = proof.verify();
        assert!(matches!(result, Err(ProofError::InvalidPath(2, 1))));
    }

    #[test]
    fn test_proof_verify_invalid_hex() {
        let proof = NomtProof {
            leaf_hash: "not-hex!".to_string(),
            siblings: vec![],
            path: vec![],
            root: "root".to_string(),
        };

        let result = proof.verify();
        assert!(matches!(result, Err(ProofError::InvalidHash(_))));
    }

    #[test]
    fn test_hash_metadata() {
        let meta = HashMetadata::new("report", "report-123")
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            .with_description("Quarterly financial report");

        assert_eq!(meta.data_type, "report");
        assert_eq!(meta.data_id, "report-123");
        assert_eq!(
            meta.description,
            Some("Quarterly financial report".to_string())
        );
    }

    #[test]
    fn test_compute_hash() {
        let hash1 = compute_hash("hello world");
        let hash2 = compute_hash("hello world");
        let hash3 = compute_hash("different data");

        assert_eq!(hash1, hash2); // Deterministic
        assert_ne!(hash1, hash3); // Different data = different hash
        assert_eq!(hash1.len(), 64); // Blake3 produces 32 bytes = 64 hex chars
    }

    #[test]
    fn test_proof_binary_conversion() {
        // P1 FIX: Use valid hex strings for all fields (hex only contains 0-9, a-f)
        let proof = NomtProof::new(
            "abcd1234",                   // Valid hex
            vec!["deadbeef".to_string()], // Valid hex
            vec![false],
            "cafebabe", // Valid hex (changed from root5678)
        );

        let binary = proof.to_binary();
        let recovered = binary.to_string_format().unwrap();

        assert_eq!(proof.leaf_hash, recovered.leaf_hash);
        assert_eq!(proof.siblings, recovered.siblings);
        assert_eq!(proof.path, recovered.path);
        assert_eq!(proof.root, recovered.root);
    }

    #[test]
    fn test_verify_success_single_level() {
        // Create a genuine 1-level Merkle proof
        let leaf_data = b"test data";
        let leaf_hash = compute_hash(leaf_data);
        let sibling_data = b"sibling data";
        let sibling_hash = compute_hash(sibling_data);

        // Compute root: leaf is on the right, so root = hash(sibling || leaf)
        let mut combined = hex::decode(&sibling_hash).unwrap();
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        combined.extend_from_slice(&hex::decode(&leaf_hash).unwrap());
        let root = hex::encode(blake3::hash(&combined).as_bytes());

        let proof = NomtProof::new(
            &leaf_hash,
            vec![sibling_hash],
            vec![true], // leaf is on the right
            &root,
        );

        assert!(proof.verify().unwrap());
    }

    #[test]
    fn test_verify_failure_wrong_root() {
        let leaf_hash = compute_hash(b"test");
        let sibling_hash = compute_hash(b"sibling");

        let proof = NomtProof::new(
            &leaf_hash,
            vec![sibling_hash],
            vec![true],
            "0000000000000000000000000000000000000000000000000000000000000000",
        );

        assert!(!proof.verify().unwrap());
    }

    #[test]
    fn test_verify_invalid_sibling_hex() {
        let leaf_hash = compute_hash(b"test");
        let proof = NomtProof::new(
            &leaf_hash,
            vec!["not-hex!".to_string()],
            vec![true],
            "abcd1234",
        );

        let result = proof.verify();
        assert!(matches!(result, Err(ProofError::InvalidHash(_))));
    }

    #[test]
    fn test_empty_proof() {
        // Empty proof: no siblings, no path → leaf_hash should equal root
        let leaf_hash = compute_hash(b"standalone");
        let proof = NomtProof::new(&leaf_hash, vec![], vec![], &leaf_hash);

        assert!(proof.verify().unwrap());
        assert_eq!(proof.depth(), 0);
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    }

    #[test]
    fn test_display_short_hash() {
        let proof = NomtProof::new("ab", vec![], vec![], "cd");
        let display = format!("{proof}");
        assert!(display.contains("NomtProof"));
        assert!(display.contains("depth=0"));
    }

    #[test]
    fn test_nomt_proof_serde_roundtrip() {
        let proof = NomtProof::new(
            "abcd1234",
            vec!["deadbeef".to_string(), "cafebabe".to_string()],
            vec![true, false],
            "root0000",
        );
        let json = serde_json::to_string(&proof).unwrap();
        let deserialized: NomtProof = serde_json::from_str(&json).unwrap();
        assert_eq!(proof, deserialized);
    }

    #[test]
    fn test_hash_metadata_serde_roundtrip() {
        let meta = HashMetadata::new("report", "r-123").with_description("test desc");
        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: HashMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.data_type, deserialized.data_type);
        assert_eq!(meta.data_id, deserialized.data_id);
        assert_eq!(meta.description, deserialized.description);
    }

    #[test]
    fn test_storage_stats_default() {
        let stats = StorageStats::default();
        assert_eq!(stats.total_items, 0);
        assert!(stats.current_root.is_empty());
        assert!(stats.backend_type.is_empty());
        assert!(stats.last_updated.is_none());
    }

    #[test]
    fn test_store_request_serde() {
        let req = StoreRequest {
            data: "hello".to_string(),
            metadata: Some(HashMetadata::new("audit", "a-1")),
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: StoreRequest = serde_json::from_str(&json).unwrap();
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        assert_eq!(req.data, deserialized.data);
    }

    #[test]
    fn test_verify_response_serde() {
        let resp = VerifyResponse {
            verified: true,
            proof: Some(NomtProof::new("leaf", vec![], vec![], "root")),
            current_root: "root".to_string(),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: VerifyResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp.verified, deserialized.verified);
        assert_eq!(resp.current_root, deserialized.current_root);
    }

    #[test]
    fn test_proof_binary_verify_success() {
        let leaf_data = b"test data";
        let leaf_hash = compute_hash(leaf_data);
        let sibling_data = b"sibling data";
        let sibling_hash = compute_hash(sibling_data);

        let mut combined = hex::decode(&sibling_hash).unwrap();
        combined.extend_from_slice(&hex::decode(&leaf_hash).unwrap());
        let root = hex::encode(blake3::hash(&combined).as_bytes());

        let proof = NomtProof::new(&leaf_hash, vec![sibling_hash], vec![true], &root);
        let binary = proof.to_binary();
        assert!(binary.verify().unwrap());
    }

    #[test]
    fn test_compute_hash_empty_data() {
        let hash = compute_hash("");
        assert_eq!(hash.len(), 64);
        assert_ne!(hash, "0".repeat(64));
    }

    #[test]
    fn test_proof_error_display() {
        let err = ProofError::InvalidHash("bad".to_string());
        assert!(err.to_string().contains("bad"));

        let err2 = ProofError::InvalidPath(2, 1);
        assert!(err2.to_string().contains('2'));
        assert!(err2.to_string().contains('1'));
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
