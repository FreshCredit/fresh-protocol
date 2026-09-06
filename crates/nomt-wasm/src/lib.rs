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
        let leaf_hash = hex::decode(leaf_hash_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid leaf_hash hex: {e}")))?;

        let mut siblings = Vec::new();
        for sibling in siblings_hex {
            let sibling_str = sibling
                .as_string()
                .ok_or_else(|| JsValue::from_str("Sibling must be a string"))?;
            let sibling_bytes = hex::decode(&sibling_str)
                .map_err(|e| JsValue::from_str(&format!("Invalid sibling hex: {e}")))?;
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            siblings.push(sibling_bytes);
        }

        let path_bool: Vec<bool> = path.iter().map(|&b| b != 0).collect();

        let root = hex::decode(root_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid root hex: {e}")))?;

        Ok(Self {
            leaf_hash,
            siblings,
            path: path_bool,
            root,
        })
    }

    /// Verify the proof against the expected root
    /// Returns true if the proof is valid
    #[wasm_bindgen]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn verify(&self, expected_root_hex: &str) -> Result<bool, JsValue> {
        let expected_root = hex::decode(expected_root_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid expected_root hex: {e}")))?;

        // Reconstruct root from leaf and siblings
        let mut current = self.leaf_hash.clone();
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001

        for (i, sibling) in self.siblings.iter().enumerate() {
            let is_right = self.path.get(i).copied().unwrap_or(false);

            // Hash the pair in the correct order
            let mut hasher = Sha256::new();
            if is_right {
                hasher.update(sibling);
                hasher.update(&current);
            } else {
                hasher.update(&current);
                hasher.update(sibling);
            }
            current = hasher.finalize().to_vec();
        }

        Ok(current == expected_root)
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

/// Compute SHA-256 hash of data (utility function)
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
        let leaf = "abcd".repeat(8); // 32 hex chars = 16 bytes
        let root = "1234".repeat(8);
        let proof = NomtProof::new(&leaf, vec![], &[], &root);
        assert!(proof.is_ok());
    }

    #[test]
    fn test_nomt_proof_getters() {
        let leaf = "aabbccdd".repeat(4);
        let root = "11223344".repeat(4);
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        let proof = NomtProof::new(&leaf, vec![], &[], &root).unwrap();
        assert_eq!(proof.leaf_hash_hex(), leaf);
        assert_eq!(proof.root_hex(), root);
    }

    #[test]
    fn test_nomt_proof_verify_leaf_equals_root() {
        // With no siblings, the leaf hash IS the root
        let leaf = hex::encode(b"leaf");
        let proof = NomtProof::new(&leaf, vec![], &[], &leaf).unwrap();
        assert!(proof.verify(&leaf).unwrap());
    }

    #[test]
    fn test_nomt_proof_verify_wrong_expected_root() {
        let leaf = hex::encode(b"leaf");
        let proof = NomtProof::new(&leaf, vec![], &[], &leaf).unwrap();
        // With no siblings, current == leaf, so verify returns false for a different expected root
        let other = hex::encode(b"other");
        assert!(!proof.verify(&other).unwrap());
    }

    #[test]
    fn test_verify_nomt_proof_leaf_equals_root() {
        let leaf = hex::encode(b"data");
        let result = verify_nomt_proof(&leaf, vec![], &[], &leaf);
        assert!(result.unwrap());
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
