// TAG: surface=blockchain owner=blockchain-team rule=BC-001
//! Storage backends for NOMT server

use async_trait::async_trait;
use freshcredit_nomt_core::{
    compute_hash, HashMetadata, NomtProof, StorageStats, StoreRequest, StoreResponse,
    VerifyRequest, VerifyResponse,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

use crate::{ServerError, Storage};

/// Storage backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackend {
    /// Legacy JSON-based storage
    Legacy,
    /// Native NOMT library storage
    Native,
}

/// Legacy JSON-based storage (NFS-compatible)
#[derive(Debug)]
pub struct LegacyStorage {
    data_dir: PathBuf,
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    state: Arc<RwLock<StorageState>>,
}

#[derive(Debug, Default)]
struct StorageState {
    items: HashMap<String, StoredItem>,
    current_root: String,
    total_count: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct StoredItem {
    hash: String,
    metadata: Option<HashMetadata>,
    proof: Option<NomtProof>,
}

impl LegacyStorage {
    /// Create a new legacy storage instance
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, ServerError> {
        let data_dir = path.as_ref().to_path_buf();

        Ok(Self {
            data_dir,
            state: Arc::new(RwLock::new(StorageState::default())),
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        })
    }

    /// Load state from disk
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn load(&self) -> Result<(), ServerError> {
        let state_file = self.data_dir.join("state.json");

        if state_file.exists() {
            let data = fs::read_to_string(&state_file).await?;
            let items: HashMap<String, StoredItem> = serde_json::from_str(&data)?;

            let mut state = self.state.write().await;
            state.total_count = items.len() as u64;
            state.items = items;

            // Compute root from items (simplified)
            if !state.items.is_empty() {
                let hashes: Vec<_> = state.items.values().map(|i| i.hash.clone()).collect();
                state.current_root = compute_root(&hashes);
            }
        }

        Ok(())
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    /// Save state to disk
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    // Allow: the read guard is deliberately held across the `fs::write` below so
    // the serialized snapshot and the in-memory state cannot interleave with a
    // concurrent `store` (blockchain anchoring consistency). Tightening the drop
    // would narrow that critical section.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn save(&self) -> Result<(), ServerError> {
        let state_file = self.data_dir.join("state.json");

        // Ensure directory exists
        fs::create_dir_all(&self.data_dir).await?;

        let state = self.state.read().await;
        let data = serde_json::to_string_pretty(&state.items)?;
        fs::write(&state_file, data).await?;

        Ok(())
    }
}

#[async_trait]
impl Storage for LegacyStorage {
    async fn store(&self, request: StoreRequest) -> Result<StoreResponse, ServerError> {
        let hash = compute_hash(&request.data);

        // Generate a simple proof (in production, this would be a real Merkle proof)
        let proof = NomtProof {
            leaf_hash: hash.clone(),
            siblings: vec![], // Simplified
            path: vec![],     // Simplified
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            root: hash.clone(), // Simplified
        };

        let item = StoredItem {
            hash: hash.clone(),
            metadata: request.metadata.clone(),
            proof: Some(proof.clone()),
        };

        let mut state = self.state.write().await;
        state.items.insert(hash.clone(), item);
        state.total_count += 1;
        state.current_root = compute_root(
            &state
                .items
                .values()
                .map(|i| i.hash.clone())
                .collect::<Vec<_>>(),
        );

        drop(state);
        self.save().await?;

        Ok(StoreResponse {
            hash: hash.clone(),
            root: hash, // Simplified
            metadata: request.metadata,
        })
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    }

    // Allow: the explicit match keeps the found/not-found verification branches
    // symmetric and auditable for anchoring review; folding them into
    // `map_or_else` closures would obscure the two VerifyResponse shapes.
    #[allow(clippy::option_if_let_else)]
    async fn verify(&self, request: VerifyRequest) -> Result<VerifyResponse, ServerError> {
        let state = self.state.read().await;

        match state.items.get(&request.hash) {
            Some(item) => Ok(VerifyResponse {
                verified: true,
                proof: item.proof.clone(),
                current_root: state.current_root.clone(),
                error: None,
            }),
            None => Ok(VerifyResponse {
                verified: false,
                proof: None,
                current_root: state.current_root.clone(),
                error: Some("Hash not found".to_string()),
            }),
        }
    }

    async fn stats(&self) -> Result<StorageStats, ServerError> {
        let state = self.state.read().await;

        Ok(StorageStats {
            total_items: state.total_count,
            current_root: state.current_root.clone(),
            last_updated: Some(chrono::Utc::now()),
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            backend_type: "legacy".to_string(),
        })
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    async fn current_root(&self) -> Result<String, ServerError> {
        let state = self.state.read().await;
        Ok(state.current_root.clone())
    }
}

/// Native NOMT library storage
#[cfg(feature = "native")]
#[derive(Debug)]
pub struct NativeStorage {
    #[allow(dead_code)]
    path: PathBuf,
}

#[cfg(feature = "native")]
impl NativeStorage {
    /// Create a new native storage instance
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, ServerError> {
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        Ok(Self {
            path: path.as_ref().to_path_buf(),
        })
    }
}

#[cfg(feature = "native")]
#[async_trait]
impl Storage for NativeStorage {
    async fn store(&self, _request: StoreRequest) -> Result<StoreResponse, ServerError> {
        // CRITICAL: Native NOMT storage is not yet implemented.
        // The NOMT (Nearly Optimal Merkle Trie) library integration is pending.
        //
        // Current behavior: Falls back to LegacyStorage which provides
        // cryptographically weak proofs (empty siblings, root=leaf).
        //
        // TODO: Implement native NOMT storage using the nomt crate once available.
        // See: https://github.com/thrumdev/nomt
        //
        // For production use, consider:
        // 1. Using a proper Merkle tree implementation
        // 2. Ensuring proofs have real sibling hashes
        // 3. Validating the Merkle root computation
        Err(ServerError::NotImplemented(
            "Native NOMT storage requires nomt library integration. \
             Use legacy storage or implement native storage. \
             See CRIT-003 in BLOCKCHAIN_HANDOFF.md"
                .to_string(),
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        ))
    }

    async fn verify(&self, _request: VerifyRequest) -> Result<VerifyResponse, ServerError> {
        Err(ServerError::NotImplemented(
            "Native NOMT storage requires nomt library integration.".to_string(),
        ))
    }

    async fn stats(&self) -> Result<StorageStats, ServerError> {
        Err(ServerError::NotImplemented(
            "Native NOMT storage requires nomt library integration.".to_string(),
        ))
    }

    async fn current_root(&self) -> Result<String, ServerError> {
        Err(ServerError::NotImplemented(
            "Native NOMT storage requires nomt library integration.".to_string(),
        ))
    }
}

/// Compute a root hash from a list of leaf hashes
fn compute_root(hashes: &[String]) -> String {
    if hashes.is_empty() {
        return compute_hash("empty");
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    if hashes.len() == 1 {
        return hashes[0].clone();
    }

    // Simple Merkle tree root computation
    let combined: String = hashes.iter().cloned().collect();
    compute_hash(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_backend_variants() {
        assert_eq!(StorageBackend::Legacy, StorageBackend::Legacy);
        assert_ne!(StorageBackend::Legacy, StorageBackend::Native);
        assert_eq!(format!("{:?}", StorageBackend::Native), "Native");
    }

    #[test]
    fn test_compute_root_empty() {
        let root = compute_root(&[]);
        assert!(!root.is_empty());
    }

    #[test]
    fn test_compute_root_single() {
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        let root = compute_root(&["hash1".to_string()]);
        assert_eq!(root, "hash1");
    }

    #[test]
    fn test_compute_root_multiple() {
        let root = compute_root(&["hash1".to_string(), "hash2".to_string()]);
        assert_ne!(root, "hash1");
        assert_ne!(root, "hash2");
    }

    #[test]
    fn test_legacy_storage_new() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LegacyStorage::new(tmp.path()).unwrap();
        assert!(storage.data_dir.exists());
    }

    #[tokio::test]
    async fn test_legacy_storage_store_and_verify() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LegacyStorage::new(tmp.path()).unwrap();

        let req = StoreRequest {
            data: "test data".to_string(),
            metadata: Some(HashMetadata::new("report", "r-1")),
        };

        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        let resp = storage.store(req.clone()).await.unwrap();
        assert!(!resp.hash.is_empty());
        assert_eq!(resp.metadata.as_ref().unwrap().data_type, "report");

        // Verify the stored hash
        let verify_req = VerifyRequest {
            hash: resp.hash,
            expected_root: None,
        };
        let verify_resp = storage.verify(verify_req).await.unwrap();
        assert!(verify_resp.verified);
        assert!(verify_resp.proof.is_some());
        assert!(!verify_resp.current_root.is_empty());
    }

    #[tokio::test]
    async fn test_legacy_storage_verify_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LegacyStorage::new(tmp.path()).unwrap();

        let verify_req = VerifyRequest {
            hash: "nonexistent".to_string(),
            expected_root: None,
        };
        let resp = storage.verify(verify_req).await.unwrap();
        assert!(!resp.verified);
        assert!(resp.error.is_some());
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001

    #[tokio::test]
    async fn test_legacy_storage_stats() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LegacyStorage::new(tmp.path()).unwrap();

        let stats = storage.stats().await.unwrap();
        assert_eq!(stats.total_items, 0);
        assert_eq!(stats.backend_type, "legacy");
        assert!(stats.current_root.is_empty());

        // Store something
        let req = StoreRequest {
            data: "hello".to_string(),
            metadata: None,
        };
        storage.store(req).await.unwrap();

        let stats = storage.stats().await.unwrap();
        assert_eq!(stats.total_items, 1);
        assert!(!stats.current_root.is_empty());
        assert!(stats.last_updated.is_some());
    }

    #[tokio::test]
    async fn test_legacy_storage_current_root() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LegacyStorage::new(tmp.path()).unwrap();
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001

        let root = storage.current_root().await.unwrap();
        assert!(root.is_empty());

        let req = StoreRequest {
            data: "world".to_string(),
            metadata: None,
        };
        storage.store(req).await.unwrap();

        let root = storage.current_root().await.unwrap();
        assert!(!root.is_empty());
    }

    #[tokio::test]
    async fn test_legacy_storage_save_and_load() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LegacyStorage::new(tmp.path()).unwrap();

        let req = StoreRequest {
            data: "persist me".to_string(),
            metadata: Some(HashMetadata::new("audit", "a-1")),
        };
        let resp = storage.store(req).await.unwrap();
        let hash = resp.hash;

        // Save to disk
        storage.save().await.unwrap();
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001

        // Create new instance pointing to same dir and load
        let loaded = LegacyStorage::new(tmp.path()).unwrap();
        loaded.load().await.unwrap();

        let verify_req = VerifyRequest {
            hash: hash.clone(),
            expected_root: None,
        };
        let verify_resp = loaded.verify(verify_req).await.unwrap();
        assert!(verify_resp.verified);

        let stats = loaded.stats().await.unwrap();
        assert_eq!(stats.total_items, 1);
    }

    #[tokio::test]
    async fn test_legacy_storage_load_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = LegacyStorage::new(tmp.path()).unwrap();

        // Load on empty directory should succeed without error
        storage.load().await.unwrap();

        let stats = storage.stats().await.unwrap();
        assert_eq!(stats.total_items, 0);
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001

    #[cfg(feature = "native")]
    #[tokio::test]
    async fn test_native_storage_not_implemented() {
        let tmp = tempfile::tempdir().unwrap();
        let storage = NativeStorage::new(tmp.path()).unwrap();

        let req = StoreRequest {
            data: "test".to_string(),
            metadata: None,
        };
        let result = storage.store(req).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not implemented"));

        let verify_req = VerifyRequest {
            hash: "h".to_string(),
            expected_root: None,
        };
        let result = storage.verify(verify_req).await;
        assert!(result.is_err());

        let result = storage.stats().await;
        assert!(result.is_err());

        let result = storage.current_root().await;
        assert!(result.is_err());
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
