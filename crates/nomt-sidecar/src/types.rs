// TAG: surface=blockchain owner=blockchain-team rule=BC-001
//! Type definitions for NOMT sidecar API
//!
//! These types define the request/response structures for the NOMT sidecar HTTP API.
//! The sidecar stores report data off-chain and generates Merkle proofs for verification.

use serde::{Deserialize, Serialize};

/// Request to store report data in NOMT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreRequest {
    /// User ID who owns the report
    pub user_id: String,
    /// Type of report (e.g., "`PlaidFinancial`", "`IdentityVerification`")
    pub report_type: String,
    /// Raw report data (will be hashed)
    pub data: Vec<u8>,
}

/// Response from storing report data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreResponse {
    /// SHA-256 hash of the stored data
    pub hash: String,
    /// NOMT root hash after insertion
    pub nomt_root: String,
    /// Merkle proof for the stored data
    pub proof: NomtProof,
    /// Timestamp of storage
    pub timestamp: String,
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}

/// Request to verify a hash
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct VerifyRequest {
    /// Hash to verify
    pub hash: String,
    /// Optional: expected NOMT root to verify against
    pub expected_root: Option<String>,
}

/// Response from hash verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResponse {
    /// Whether the hash exists in NOMT storage
    pub exists: bool,
    /// Whether the hash is verified against the current root
    pub verified: bool,
    /// Current NOMT root hash
    pub nomt_root: String,
    /// User ID associated with the hash (if found)
    pub user_id: Option<String>,
    /// Report type (if found)
    pub report_type: Option<String>,
    /// Timestamp when the data was stored
    pub timestamp: Option<String>,
}

/// Merkle proof for a stored hash
// TAG: surface=blockchain owner=blockchain-team rule=BC-001
///
/// NOMT uses a binary Merkle trie, so the proof consists of:
/// - The leaf hash (the stored data hash)
/// - Sibling hashes along the path to the root
/// - The path direction (left/right) at each level
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Verify this proof against an expected root
    ///
    /// Uses Blake3 for consistency with freshcredit-nomt-core.
    pub fn verify(&self, expected_root: &str) -> bool {
        // Use the canonical verification from freshcredit-nomt-core
        self.verify_canonical()
            .is_ok_and(|valid| valid && self.root == expected_root)
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001

    /// Verify using the canonical freshcredit-nomt-core implementation
    fn verify_canonical(&self) -> Result<bool, freshcredit_nomt_core::ProofError> {
        let canonical = freshcredit_nomt_core::NomtProof {
            leaf_hash: self.leaf_hash.clone(),
            siblings: self.siblings.clone(),
            path: self.path.clone(),
            root: self.root.clone(),
        };
        canonical.verify()
    }
}

/// Metadata stored alongside each hash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashMetadata {
    /// User ID who owns the data
    pub user_id: String,
    /// Type of report
    pub report_type: String,
    /// Timestamp when stored
    pub timestamp: String,
    /// Size of original data in bytes
    pub data_size: usize,
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    /// Total number of stored hashes
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    pub total_hashes: u64,
    /// Total data size in bytes
    pub total_data_bytes: u64,
    /// Current NOMT root hash
    pub current_root: String,
    /// Number of unique users
    pub unique_users: u64,
    /// Storage efficiency ratio
    pub efficiency_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_request_creation() {
        let request = StoreRequest {
            user_id: "user-123".to_string(),
            report_type: "PlaidFinancial".to_string(),
            data: vec![1, 2, 3, 4, 5],
        };

        assert_eq!(request.user_id, "user-123");
        assert_eq!(request.report_type, "PlaidFinancial");
        assert_eq!(request.data.len(), 5);
    }

    #[test]
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    fn test_store_response_creation() {
        let proof = NomtProof {
            leaf_hash: "abc123".to_string(),
            siblings: vec!["def456".to_string()],
            path: vec![true],
            root: "root789".to_string(),
        };

        let response = StoreResponse {
            hash: "abc123".to_string(),
            nomt_root: "root789".to_string(),
            proof,
            timestamp: "2024-01-15T10:30:00Z".to_string(),
        };

        assert_eq!(response.hash, "abc123");
        assert_eq!(response.nomt_root, "root789");
    }

    #[test]
    fn test_verify_request_creation() {
        let request = VerifyRequest {
            hash: "abc123".to_string(),
            expected_root: Some("root789".to_string()),
        };

        assert_eq!(request.hash, "abc123");
        assert_eq!(request.expected_root, Some("root789".to_string()));
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    #[test]
    fn test_verify_request_without_expected_root() {
        let request = VerifyRequest {
            hash: "abc123".to_string(),
            expected_root: None,
        };

        assert!(request.expected_root.is_none());
    }

    #[test]
    fn test_verify_response_found() {
        let response = VerifyResponse {
            exists: true,
            verified: true,
            nomt_root: "root789".to_string(),
            user_id: Some("user-123".to_string()),
            report_type: Some("PlaidFinancial".to_string()),
            timestamp: Some("2024-01-15T10:30:00Z".to_string()),
        };

        assert!(response.exists);
        assert!(response.verified);
        assert_eq!(response.user_id, Some("user-123".to_string()));
    }

    #[test]
    fn test_verify_response_not_found() {
        let response = VerifyResponse {
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            exists: false,
            verified: false,
            nomt_root: "root789".to_string(),
            user_id: None,
            report_type: None,
            timestamp: None,
        };

        assert!(!response.exists);
        assert!(!response.verified);
        assert!(response.user_id.is_none());
    }

    #[test]
    fn test_nomt_proof_creation() {
        let proof = NomtProof {
            leaf_hash: "abc123".to_string(),
            siblings: vec!["def456".to_string(), "ghi789".to_string()],
            path: vec![true, false],
            root: "root123".to_string(),
        };

        assert_eq!(proof.leaf_hash, "abc123");
        assert_eq!(proof.siblings.len(), 2);
        assert_eq!(proof.path.len(), 2);
        assert!(proof.path[0]);
        assert!(!proof.path[1]);
    }

    #[test]
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    fn test_hash_metadata_creation() {
        let metadata = HashMetadata {
            user_id: "user-123".to_string(),
            report_type: "PlaidFinancial".to_string(),
            timestamp: "2024-01-15T10:30:00Z".to_string(),
            data_size: 1024,
        };

        assert_eq!(metadata.user_id, "user-123");
        assert_eq!(metadata.data_size, 1024);
    }

    #[test]
    fn test_storage_stats_creation() {
        let stats = StorageStats {
            total_hashes: 100,
            total_data_bytes: 1_024_000,
            current_root: "root123".to_string(),
            unique_users: 50,
            efficiency_ratio: 0.95,
        };

        assert_eq!(stats.total_hashes, 100);
        assert_eq!(stats.unique_users, 50);
        assert!((stats.efficiency_ratio - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_nomt_proof_serde_roundtrip() {
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        let original = NomtProof {
            leaf_hash: "abcd1234".to_string(),
            siblings: vec!["sib1".to_string(), "sib2".to_string()],
            path: vec![true, false],
            root: "root9876".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: NomtProof = serde_json::from_str(&json).unwrap();
        assert_eq!(original.leaf_hash, deserialized.leaf_hash);
        assert_eq!(original.siblings, deserialized.siblings);
        assert_eq!(original.path, deserialized.path);
        assert_eq!(original.root, deserialized.root);
    }

    #[test]
    fn test_hash_metadata_serde_roundtrip() {
        let original = HashMetadata {
            user_id: "user-456".to_string(),
            report_type: "IdentityVerification".to_string(),
            timestamp: "2024-06-01T12:00:00Z".to_string(),
            data_size: 2048,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: HashMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(original.user_id, deserialized.user_id);
        assert_eq!(original.report_type, deserialized.report_type);
        assert_eq!(original.timestamp, deserialized.timestamp);
        assert_eq!(original.data_size, deserialized.data_size);
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    #[test]
    fn test_storage_stats_serde_roundtrip() {
        let original = StorageStats {
            total_hashes: 42,
            total_data_bytes: 12345,
            current_root: "rootabc".to_string(),
            unique_users: 7,
            efficiency_ratio: 0.88,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: StorageStats = serde_json::from_str(&json).unwrap();
        assert_eq!(original.total_hashes, deserialized.total_hashes);
        assert_eq!(original.total_data_bytes, deserialized.total_data_bytes);
        assert_eq!(original.current_root, deserialized.current_root);
        assert_eq!(original.unique_users, deserialized.unique_users);
        assert!((original.efficiency_ratio - deserialized.efficiency_ratio).abs() < 0.001);
    }

    #[test]
    fn test_verify_request_serde_roundtrip() {
        let original = VerifyRequest {
            hash: "hash1234".to_string(),
            expected_root: Some("root5678".to_string()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: VerifyRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(original.hash, deserialized.hash);
        assert_eq!(original.expected_root, deserialized.expected_root);

        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        let without_root = VerifyRequest {
            hash: "hash9999".to_string(),
            expected_root: None,
        };
        let json2 = serde_json::to_string(&without_root).unwrap();
        let deserialized2: VerifyRequest = serde_json::from_str(&json2).unwrap();
        assert!(deserialized2.expected_root.is_none());
    }

    #[test]
    fn test_store_response_serde_roundtrip() {
        let proof = NomtProof {
            leaf_hash: "leaf0001".to_string(),
            siblings: vec!["sib0001".to_string()],
            path: vec![false],
            root: "root0001".to_string(),
        };
        let original = StoreResponse {
            hash: "hash0001".to_string(),
            nomt_root: "root0001".to_string(),
            proof,
            timestamp: "2024-12-25T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: StoreResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(original.hash, deserialized.hash);
        assert_eq!(original.nomt_root, deserialized.nomt_root);
        assert_eq!(original.timestamp, deserialized.timestamp);
        assert_eq!(original.proof.leaf_hash, deserialized.proof.leaf_hash);
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
