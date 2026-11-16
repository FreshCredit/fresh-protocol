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

/// RFC 6962 §2.1 domain-separation byte for Merkle leaves.
pub const LEAF_DOMAIN_PREFIX: u8 = 0x00;

/// RFC 6962 §2.1 domain-separation byte for Merkle internal nodes.
pub const NODE_DOMAIN_PREFIX: u8 = 0x01;

/// Required byte length for 256-bit BLAKE3 digests.
const DIGEST_LEN: usize = 32;

/// Compute a domain-separated RFC 6962 leaf hash (`BLAKE3(0x00 || data)`).
#[must_use]
pub fn compute_leaf_hash(data: impl AsRef<[u8]>) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[LEAF_DOMAIN_PREFIX]);
    hasher.update(data.as_ref());
    hex::encode(hasher.finalize().as_bytes())
}

/// Compute a key/namespace-bound RFC 6962 leaf hash:
/// `BLAKE3(0x00 || u32_be(namespace.len()) || namespace || data)`.
/// Prevents an attacker from substituting a valid proof for key A as proof for key B.
#[must_use]
pub fn compute_bound_leaf_hash(namespace: &str, data: impl AsRef<[u8]>) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[LEAF_DOMAIN_PREFIX]);
    let ns_bytes = namespace.as_bytes();
    let ns_len = u32::try_from(ns_bytes.len()).unwrap_or(u32::MAX);
    hasher.update(&ns_len.to_be_bytes());
    hasher.update(ns_bytes);
    hasher.update(data.as_ref());
    hex::encode(hasher.finalize().as_bytes())
}

/// Compute a deterministic RFC 6962 binary Merkle root over sorted 32-byte hex leaves.
#[must_use]
pub fn compute_merkle_root(ordered_leaves: &[String]) -> String {
    if ordered_leaves.is_empty() {
        return compute_leaf_hash(b"empty");
    }
    if ordered_leaves.len() == 1 {
        return ordered_leaves[0].clone();
    }
    let mut level: Vec<[u8; DIGEST_LEN]> = ordered_leaves
        .iter()
        .map(|h| decode_digest_32(h).unwrap_or_else(|_| *blake3::hash(h.as_bytes()).as_bytes()))
        .collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            if pair.len() == 2 {
                next.push(hash_internal_nodes(&pair[0], &pair[1]));
            } else {
                next.push(pair[0]);
            }
        }
        level = next;
    }
    hex::encode(level[0])
}

/// Build an RFC 6962 binary Merkle inclusion proof for `target_leaf` over `ordered_leaves`.
#[must_use]
pub fn build_merkle_proof(ordered_leaves: &[String], target_leaf: &str) -> Option<NomtProof> {
    let mut idx = ordered_leaves.iter().position(|h| h == target_leaf)?;
    let mut level: Vec<[u8; DIGEST_LEN]> = ordered_leaves
        .iter()
        .map(|h| decode_digest_32(h).ok())
        .collect::<Option<Vec<_>>>()?;

    let mut siblings = Vec::new();
    let mut path = Vec::new();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for (pair_idx, pair) in level.chunks(2).enumerate() {
            if pair.len() == 2 {
                if idx / 2 == pair_idx {
                    let is_right = idx % 2 == 1;
                    let sibling = if is_right { pair[0] } else { pair[1] };
                    siblings.push(hex::encode(sibling));
                    path.push(is_right);
                }
                next.push(hash_internal_nodes(&pair[0], &pair[1]));
            } else {
                next.push(pair[0]);
            }
        }
        idx /= 2;
        level = next;
    }
    let root = hex::encode(level[0]);
    Some(NomtProof::new(target_leaf, siblings, path, root))
}

/// Compute a domain-separated RFC 6962 internal node hash (`BLAKE3(0x01 || left || right)`).
#[must_use]
pub fn hash_internal_nodes(left: &[u8; DIGEST_LEN], right: &[u8; DIGEST_LEN]) -> [u8; DIGEST_LEN] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[NODE_DOMAIN_PREFIX]);
    hasher.update(left);
    hasher.update(right);
    *hasher.finalize().as_bytes()
}

fn decode_digest_32(hex_str: &str) -> Result<[u8; DIGEST_LEN], ProofError> {
    let bytes = hex::decode(hex_str).map_err(|_| ProofError::InvalidHash(hex_str.to_string()))?;
    if bytes.len() != DIGEST_LEN {
        return Err(ProofError::InvalidHash(format!(
            "expected {DIGEST_LEN}-byte digest, got {} bytes",
            bytes.len()
        )));
    }
    let mut out = [0u8; DIGEST_LEN];
    out.copy_from_slice(&bytes);
    Ok(out)
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

        let mut current_hash = decode_digest_32(&self.leaf_hash)?;
        let expected_root = decode_digest_32(&self.root)?;

        for (i, (sibling, is_right)) in self.siblings.iter().zip(&self.path).enumerate() {
            let sibling_bytes = decode_digest_32(sibling)?;

            // RFC 6962 §2.1 domain-separated internal node hash: H(0x01 || left || right)
            current_hash = if *is_right {
                hash_internal_nodes(&sibling_bytes, &current_hash)
            } else {
                hash_internal_nodes(&current_hash, &sibling_bytes)
            };

            tracing::debug!(
                level = i,
                hash = hex::encode(current_hash),
                "Computed intermediate hash"
            );
        }

        Ok(current_hash == expected_root)
    }

    /// Verify this proof against an external trusted `expected_root` and `expected_leaf_hash`
    /// (preventing self-signed `proof.root` or cross-key leaf substitution).
    /// # Errors
    ///
    /// Returns `ProofError` if any digest is invalid or path lengths mismatch.
    pub fn verify_against(
        &self,
        expected_root: &str,
        expected_leaf_hash: &str,
    ) -> Result<bool, ProofError> {
        if self.root != expected_root || self.leaf_hash != expected_leaf_hash {
            return Ok(false);
        }
        self.verify()
    }

    /// Verify inclusion of raw `(namespace, data)` against an external trusted `expected_root`,
    /// recomputing the domain-separated RFC 6962 `0x00` bound leaf hash and optionally enforcing
    /// expected binary tree depth (`ceil_log2(leaf_count)`) so internal `0x01` nodes cannot
    /// masquerade as leaves.
    ///
    /// # Errors
    ///
    /// Returns `ProofError` if any digest is malformed or path lengths mismatch.
    pub fn verify_bound_leaf(
        &self,
        namespace: &str,
        data: &[u8],
        expected_root: &str,
        expected_leaf_count: Option<usize>,
    ) -> Result<bool, ProofError> {
        if let Some(n) = expected_leaf_count {
            if n == 0 {
                return Ok(false);
            }
            let min_depth = (usize::BITS - (n - 1).leading_zeros()) as usize;
            if n > 1 && (self.depth() < min_depth.saturating_sub(1) || self.depth() > min_depth) {
                return Ok(false);
            }
            if n == 1 && self.depth() != 0 {
                return Ok(false);
            }
        }
        let expected_leaf = compute_bound_leaf_hash(namespace, data);
        self.verify_against(expected_root, &expected_leaf)
    }

    /// Get the proof depth (number of levels)
    #[must_use]
    pub fn depth(&self) -> usize {
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        self.siblings.len()
    }

    /// Checked binary conversion (fails closed on malformed hex instead of dropping siblings).
    /// # Errors
    ///
    /// Returns `ProofError::InvalidHash` if any hex digest is invalid.
    pub fn try_to_binary(&self) -> Result<NomtProofBinary, ProofError> {
        let leaf_hash = hex::decode(&self.leaf_hash)
            .map_err(|_| ProofError::InvalidHash(self.leaf_hash.clone()))?;
        let mut siblings = Vec::with_capacity(self.siblings.len());
        for s in &self.siblings {
            siblings.push(hex::decode(s).map_err(|_| ProofError::InvalidHash(s.clone()))?);
        }
        let root =
            hex::decode(&self.root).map_err(|_| ProofError::InvalidHash(self.root.clone()))?;
        Ok(NomtProofBinary {
            leaf_hash,
            siblings,
            path: self.path.clone(),
            root,
        })
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
        // Create a genuine domain-separated 1-level Merkle proof (RFC 6962)
        let leaf_hash = compute_leaf_hash(b"test data");
        let sibling_hash = compute_leaf_hash(b"sibling data");

        let sib_arr: [u8; 32] = hex::decode(&sibling_hash).unwrap().try_into().unwrap();
        let leaf_arr: [u8; 32] = hex::decode(&leaf_hash).unwrap().try_into().unwrap();
        let root = hex::encode(hash_internal_nodes(&sib_arr, &leaf_arr));

        let proof = NomtProof::new(
            &leaf_hash,
            vec![sibling_hash],
            vec![true], // leaf is on the right
            &root,
        );

        assert!(proof.verify().unwrap());
    }

    #[test]
    fn test_verify_bound_leaf_rejects_internal_node_as_leaf_and_cross_namespace() {
        let l1 = compute_bound_leaf_hash("alice:credit", b"payload-1");
        let l2 = compute_bound_leaf_hash("bob:credit", b"payload-2");
        let l3 = compute_bound_leaf_hash("carol:credit", b"payload-3");
        let l4 = compute_bound_leaf_hash("dave:credit", b"payload-4");
        let leaves = vec![l1.clone(), l2.clone(), l3.clone(), l4.clone()];
        let root = compute_merkle_root(&leaves);
        let proof = build_merkle_proof(&leaves, &l1).unwrap();

        assert!(proof
            .verify_bound_leaf("alice:credit", b"payload-1", &root, Some(4))
            .unwrap());
        // Cross-namespace transplant must fail
        assert!(!proof
            .verify_bound_leaf("eve:credit", b"payload-1", &root, Some(4))
            .unwrap());

        // Internal-node-as-leaf attack: forge a depth-1 proof claiming internal node H(0x01||l1||l2) is a leaf
        let h1: [u8; 32] = hex::decode(&l1).unwrap().try_into().unwrap();
        let h2: [u8; 32] = hex::decode(&l2).unwrap().try_into().unwrap();
        let h3: [u8; 32] = hex::decode(&l3).unwrap().try_into().unwrap();
        let h4: [u8; 32] = hex::decode(&l4).unwrap().try_into().unwrap();
        let internal_left = hex::encode(hash_internal_nodes(&h1, &h2));
        let internal_right = hex::encode(hash_internal_nodes(&h3, &h4));
        let forged_internal_proof =
            NomtProof::new(&internal_left, vec![internal_right], vec![false], &root);
        // Raw verify() passes on forged internal node, proving why verify_bound_leaf is essential!
        assert!(forged_internal_proof.verify().unwrap());
        assert!(!forged_internal_proof
            .verify_bound_leaf("alice:credit", b"payload-1", &root, Some(4))
            .unwrap());
    }

    #[test]
    fn test_rfc6962_second_preimage_forgery_rejected() {
        // Forging a depth-0 leaf whose raw data equals (sib || leaf) must NOT match
        // an internal node hash because leaves use 0x00 prefix and nodes use 0x01.
        let sib_arr: [u8; 32] = *blake3::hash(b"\x00sibling").as_bytes();
        let leaf_arr: [u8; 32] = *blake3::hash(b"\x00leaf").as_bytes();
        let internal_root = hex::encode(hash_internal_nodes(&sib_arr, &leaf_arr));

        let mut forged_leaf_preimage = Vec::with_capacity(64);
        forged_leaf_preimage.extend_from_slice(&sib_arr);
        forged_leaf_preimage.extend_from_slice(&leaf_arr);
        let forged_leaf_hash = compute_leaf_hash(&forged_leaf_preimage);

        let forged_proof = NomtProof::new(&forged_leaf_hash, vec![], vec![], &internal_root);
        assert!(!forged_proof.verify().unwrap());

        // Short-length digests are also rejected
        let short = NomtProof::new("abcd", vec![], vec![], "abcd");
        assert!(matches!(short.verify(), Err(ProofError::InvalidHash(_))));
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
        let leaf_hash = compute_leaf_hash(b"test data");
        let sibling_hash = compute_leaf_hash(b"sibling data");

        let sib_arr: [u8; 32] = hex::decode(&sibling_hash).unwrap().try_into().unwrap();
        let leaf_arr: [u8; 32] = hex::decode(&leaf_hash).unwrap().try_into().unwrap();
        let root = hex::encode(hash_internal_nodes(&sib_arr, &leaf_arr));

        let proof = NomtProof::new(&leaf_hash, vec![sibling_hash], vec![true], &root);
        let binary = proof.try_to_binary().unwrap();
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
