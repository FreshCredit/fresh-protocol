// TAG: surface=blockchain owner=blockchain-team rule=BC-001
//! NOMT WASM - Browser-side Binary Merkle Trie proof verification
//!
//! This crate provides WebAssembly bindings for verifying NOMT Binary Merkle Trie
//! proofs in the browser. It is designed to work alongside smoldot (Patricia Trie)
//! as an ADDITIONAL verification method, not a replacement.
//!
//! Architecture:
//! - Server generates NOMT proof when storing report data
//! - Browser receives proof along with data
//! - Browser verifies proof using this WASM module
//! - Verification is trustless (cryptographic, not trusting server)
//!
//! Note: NOMT uses Binary Merkle Trie (2 children per node), which is different
//! from Substrate's Patricia Merkle Trie (16 children with path compression).

#![forbid(unsafe_code)]

use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

/// Binary Merkle Trie proof structure
/// Matches the `NomtProof` struct from nomt-sidecar
#[wasm_bindgen]
#[derive(Debug)]
pub struct NomtProof {
    /// Hash of the leaf node (the stored data)
    leaf_hash: Vec<u8>,
    /// Sibling hashes along the path to root
    siblings: Vec<Vec<u8>>,
    /// Path direction at each level (true = right, false = left)
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    path: Vec<bool>,
    /// Root hash this proof verifies against
    root: Vec<u8>,
}

#[derive(Debug)]
struct ParsedProofInputs {
    leaf_hash: Vec<u8>,
    siblings: Vec<Vec<u8>>,
    path: Vec<bool>,
    root: Vec<u8>,
}

fn parse_proof_inputs(
    leaf_hash_hex: &str,
    siblings_hex: &[String],
    path: &[u8],
    root_hex: &str,
) -> Result<ParsedProofInputs, String> {
    let leaf_hash =
        hex::decode(leaf_hash_hex).map_err(|e| format!("Invalid leaf_hash hex: {e}"))?;
    if leaf_hash.len() != 32 {
        return Err("leaf_hash must be 32 bytes".to_string());
    }

    let mut siblings = Vec::with_capacity(siblings_hex.len());
    for sibling_str in siblings_hex {
        let sibling_bytes =
            hex::decode(sibling_str).map_err(|e| format!("Invalid sibling hex: {e}"))?;
        if sibling_bytes.len() != 32 {
            return Err("Each sibling hash must be 32 bytes".to_string());
        }
        siblings.push(sibling_bytes);
    }

    let path_bool: Vec<bool> = path.iter().map(|&b| b != 0).collect();
    if path_bool.len() != siblings.len() {
        return Err("path length must match siblings length".to_string());
    }

    let root = hex::decode(root_hex).map_err(|e| format!("Invalid root hex: {e}"))?;
    if root.len() != 32 {
        return Err("root must be 32 bytes".to_string());
    }

    Ok(ParsedProofInputs {
        leaf_hash,
        siblings,
        path: path_bool,
        root,
    })
}

fn verify_proof_inner(
    leaf_hash: &[u8],
    siblings: &[Vec<u8>],
    path: &[bool],
    expected_root_hex: &str,
) -> Result<bool, String> {
    let expected_root =
        hex::decode(expected_root_hex).map_err(|e| format!("Invalid expected_root hex: {e}"))?;
    if expected_root.len() != 32 || leaf_hash.len() != 32 {
        return Err("Hashes must be 32 bytes".to_string());
    }
    if path.len() != siblings.len() {
        return Err("path length must match siblings length".to_string());
    }

    let mut current = leaf_hash.to_vec();
    for (i, sibling) in siblings.iter().enumerate() {
        if sibling.len() != 32 {
            return Err("Sibling hash must be 32 bytes".to_string());
        }
        let is_right = path[i];
        let mut hasher = blake3::Hasher::new();
        hasher.update(&[0x01]);
        if is_right {
            hasher.update(sibling);
            hasher.update(&current);
        } else {
            hasher.update(&current);
            hasher.update(sibling);
        }
        current = hasher.finalize().as_bytes().to_vec();
    }

    Ok(current == expected_root)
}

#[wasm_bindgen]
impl NomtProof {
    /// Create a new proof from JavaScript
    #[wasm_bindgen(constructor)]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn new(
        leaf_hash_hex: &str,
        siblings_hex: Vec<JsValue>,
        path: &[u8],
        root_hex: &str,
    ) -> Result<Self, JsValue> {
        let mut sibling_strs = Vec::with_capacity(siblings_hex.len());
        for sibling in siblings_hex {
            let sibling_str = sibling
                .as_string()
                .ok_or_else(|| JsValue::from_str("Sibling must be a string"))?;
            sibling_strs.push(sibling_str);
        }
        let parsed = parse_proof_inputs(leaf_hash_hex, &sibling_strs, path, root_hex)
            .map_err(|e| JsValue::from_str(&e))?;

        Ok(Self {
            leaf_hash: parsed.leaf_hash,
            siblings: parsed.siblings,
            path: parsed.path,
            root: parsed.root,
        })
    }

    /// Verify the proof against the expected root using RFC 6962 §2.1 domain-separated BLAKE3
    /// (`0x01 || left || right` for internal nodes, matching `freshcredit-nomt-core`).
    /// Returns true if the proof is valid.
    #[wasm_bindgen]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn verify(&self, expected_root_hex: &str) -> Result<bool, JsValue> {
        verify_proof_inner(
            &self.leaf_hash,
            &self.siblings,
            &self.path,
            expected_root_hex,
        )
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Verify the proof against an expected root AND an expected leaf hash (preventing cross-key substitution).
    #[wasm_bindgen]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn verify_against(
        &self,
        expected_root_hex: &str,
        expected_leaf_hex: &str,
    ) -> Result<bool, JsValue> {
        let expected_leaf = hex::decode(expected_leaf_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid expected_leaf hex: {e}")))?;
        if expected_leaf.len() != 32 {
            return Err(JsValue::from_str("expected_leaf must be 32 bytes"));
        }
        if self.leaf_hash != expected_leaf {
            return Ok(false);
        }
        self.verify(expected_root_hex)
    }

    /// Get the leaf hash as hex string
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn leaf_hash_hex(&self) -> String {
        hex::encode(&self.leaf_hash)
    }

    /// Get the root hash as hex string
    #[wasm_bindgen(getter)]
    /// # Errors
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    ///
    /// Returns an error if the operation fails.
    #[must_use]
    pub fn root_hex(&self) -> String {
        hex::encode(&self.root)
    }
}

/// Verify a NOMT proof (convenience function)
/// This is the main function called from JavaScript
#[wasm_bindgen]
/// # Errors
///
/// Returns an error if the operation fails.
pub fn verify_nomt_proof(
    leaf_hash_hex: &str,
    siblings_hex: Vec<JsValue>,
    path: &[u8],
    expected_root_hex: &str,
) -> Result<bool, JsValue> {
    let proof = NomtProof::new(leaf_hash_hex, siblings_hex, path, expected_root_hex)?;
    proof.verify(expected_root_hex)
}

/// Compute RFC 6962 §2.1 domain-separated BLAKE3 leaf hash (`0x00 || data`).
#[wasm_bindgen]
#[must_use]
pub fn blake3_leaf_hash(data: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[0x00]);
    hasher.update(data);
    hex::encode(hasher.finalize().as_bytes())
}

/// Compute key/namespace-bound RFC 6962 §2.1 BLAKE3 leaf hash
/// (`0x00 || u32_be(namespace.len()) || namespace || data`).
#[wasm_bindgen]
#[must_use]
pub fn blake3_bound_leaf_hash(namespace: &str, data: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[0x00]);
    let ns_bytes = namespace.as_bytes();
    let ns_len = u32::try_from(ns_bytes.len()).unwrap_or(u32::MAX);
    hasher.update(&ns_len.to_be_bytes());
    hasher.update(ns_bytes);
    hasher.update(data);
    hex::encode(hasher.finalize().as_bytes())
}

/// Compute SHA-256 hash of data (legacy utility function)
#[wasm_bindgen]
#[must_use]
pub fn sha256_hash(data: &[u8]) -> String {
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Get WASM module version
#[wasm_bindgen]
#[must_use]
pub fn get_version() -> String {
    "0.1.0".to_string()
}

/// Initialize the WASM module (called once on load)
#[allow(clippy::missing_const_for_fn)] // wasm_bindgen(start) entry points cannot be const fns
#[wasm_bindgen(start)]
pub fn init() {
    // WASM module initialized successfully
    // Panic hook not needed since we use Result types
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_hash() {
        let data = b"hello world";
        let hash = sha256_hash(data);
        assert_eq!(
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_get_version() {
        assert_eq!(get_version(), "0.1.0");
    }

    #[test]
    fn test_init() {
        // init is a no-op, just verify it doesn't panic
        init();
    }

    #[test]
    fn test_nomt_proof_new_with_empty_siblings() {
        let leaf = "ab".repeat(32); // 64 hex chars = 32 bytes
        let root = "12".repeat(32);
        let proof = NomtProof::new(&leaf, vec![], &[], &root);
        assert!(proof.is_ok());
    }

    #[test]
    fn test_nomt_proof_rejects_short_digest() {
        let short_leaf = "abcd".repeat(4); // 16 bytes != 32 bytes
        let root = "12".repeat(32);
        let err = parse_proof_inputs(&short_leaf, &[], &[], &root).unwrap_err();
        assert!(err.contains("32 bytes"));
    }

    #[test]
    fn test_nomt_proof_rejects_path_length_mismatch() {
        let leaf = blake3_leaf_hash(b"leaf");
        let root = blake3_leaf_hash(b"root");
        let err = parse_proof_inputs(&leaf, &[leaf.clone()], &[], &root).unwrap_err();
        assert!(err.contains("path length"));
    }

    #[test]
    fn test_nomt_proof_getters() {
        let leaf = "aabbccdd".repeat(8); // 32 bytes
        let root = "11223344".repeat(8); // 32 bytes
                                         // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        let proof = NomtProof::new(&leaf, vec![], &[], &root).unwrap();
        assert_eq!(proof.leaf_hash_hex(), leaf);
        assert_eq!(proof.root_hex(), root);
    }

    #[test]
    fn test_nomt_proof_verify_leaf_equals_root() {
        let leaf = blake3_leaf_hash(b"leaf");
        let proof = NomtProof::new(&leaf, vec![], &[], &leaf).unwrap();
        assert!(proof.verify(&leaf).unwrap());
    }

    #[test]
    fn test_nomt_proof_verify_wrong_expected_root() {
        let leaf = blake3_leaf_hash(b"leaf");
        let proof = NomtProof::new(&leaf, vec![], &[], &leaf).unwrap();
        let other = blake3_leaf_hash(b"other");
        assert!(!proof.verify(&other).unwrap());
    }

    #[test]
    fn test_nomt_proof_verify_rfc6962_blake3_internal_node() {
        let left_hex = blake3_leaf_hash(b"left-leaf");
        let right_hex = blake3_leaf_hash(b"right-leaf");
        let left_bytes = hex::decode(&left_hex).unwrap();
        let right_bytes = hex::decode(&right_hex).unwrap();
        let mut h = blake3::Hasher::new();
        h.update(&[0x01]);
        h.update(&left_bytes);
        h.update(&right_bytes);
        let root_hex = hex::encode(h.finalize().as_bytes());

        let proof = NomtProof {
            leaf_hash: left_bytes,
            siblings: vec![right_bytes],
            path: vec![false],
            root: hex::decode(&root_hex).unwrap(),
        };
        assert!(proof.verify(&root_hex).unwrap());

        let bad_dir = NomtProof {
            leaf_hash: proof.leaf_hash.clone(),
            siblings: proof.siblings.clone(),
            path: vec![true],
            root: proof.root.clone(),
        };
        assert!(!bad_dir.verify(&root_hex).unwrap());
    }

    #[test]
    fn test_verify_nomt_proof_leaf_equals_root() {
        let leaf = blake3_leaf_hash(b"data");
        let result = verify_nomt_proof(&leaf, vec![], &[], &leaf);
        assert!(result.unwrap());
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
