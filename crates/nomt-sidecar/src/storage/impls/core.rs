// TAG: surface=blockchain owner=blockchain-team rule=BC-001
use crate::storage::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, info, warn};

impl NomtStorage {
    /// Create a new NOMT storage instance with persistent storage
    pub async fn new(data_dir: &str) -> anyhow::Result<Self> {
        let path = PathBuf::from(data_dir);

        // Create data directory if it doesn't exist
        // P0-PERF-006: Using spawn_blocking for sync file operations
        if !path.exists() {
            let path_clone = path.clone();
            tokio::task::spawn_blocking(move || std::fs::create_dir_all(&path_clone)).await??;
            info!(path = %path.display(), "Created NOMT data directory");
        }

        // Try to load existing state from disk
        let state_file = path.join("nomt_state.json");
        let (hash_index, proof_index, current_root, total_hashes, total_bytes) = if state_file
            .exists()
        {
            let state_file_clone = state_file.clone();
            match tokio::task::spawn_blocking(move || std::fs::read_to_string(&state_file_clone))
                .await?
            {
                Ok(content) => match serde_json::from_str::<PersistentState>(&content) {
                    Ok(state) => {
                        let root_bytes = hex::decode(&state.current_root)
                            .ok()
                            .and_then(|b| {
                                if b.len() == 32 {
                                    let mut arr = [0u8; 32];
                                    arr.copy_from_slice(&b);
                                    Some(arr)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or([0u8; 32]);
                        info!(
                            path = %state_file.display(),
                            hashes = state.total_hashes,
                            "Loaded existing NOMT state from disk"
                        );
                        (
                            state.hash_index,
                            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
                            state.proof_index,
                            root_bytes,
                            state.total_hashes,
                            state.total_bytes,
                        )
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to parse NOMT state, starting fresh");
                        (HashMap::new(), HashMap::new(), [0u8; 32], 0, 0)
                    }
                },
                Err(e) => {
                    warn!(error = %e, "Failed to read NOMT state file, starting fresh");
                    (HashMap::new(), HashMap::new(), [0u8; 32], 0, 0)
                }
            }
        } else {
            info!(path = %path.display(), "No existing NOMT state, starting fresh");
            (HashMap::new(), HashMap::new(), [0u8; 32], 0, 0)
        };

        info!(
            path = %path.display(),
            total_hashes = total_hashes,
            root = %hex::encode(current_root),
            "NOMT storage initialized"
        );

        Ok(Self {
            data_dir: path,
            hash_index,
            proof_index,
            current_root,
            total_hashes: AtomicU64::new(total_hashes),
            total_bytes: AtomicU64::new(total_bytes),
            ready: true,
        })
    }

    /// Store report data and generate Merkle proof
    ///
    /// Implements an incremental Merkle tree where:
    /// - `new_root` = `SHA256(previous_root` || `leaf_hash`)
    /// - proof.sibling = `previous_root` (allows verification by computing hash(sibling || leaf))
    /// - `proof.is_right` = true (leaf is always on the right in our scheme)
    pub async fn store(
        &mut self,
        user_id: &str,
        report_type: &str,
        data: &[u8],
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    ) -> anyhow::Result<StoreResponse> {
        // Compute SHA-256 hash of the data
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash_bytes: [u8; 32] = hasher.finalize().into();
        let hash = hex::encode(hash_bytes);

        debug!(hash = %hash, report_type = %report_type, "Storing report data");

        // Store metadata
        let timestamp = chrono::Utc::now().to_rfc3339();
        let metadata = HashMetadata {
            user_id: user_id.to_string(),
            report_type: report_type.to_string(),
            timestamp: timestamp.clone(),
            data_size: data.len(),
        };
        self.hash_index.insert(hash.clone(), metadata);

        // Update statistics
        self.total_hashes.fetch_add(1, Ordering::Relaxed);
        self.total_bytes
            .fetch_add(data.len() as u64, Ordering::Relaxed);

        // Store the previous root as the sibling for this hash's proof
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        // This enables verification: new_root = SHA256(previous_root || leaf_hash)
        let previous_root = hex::encode(self.current_root);

        // Update root hash: new_root = SHA256(previous_root || leaf_hash)
        let mut root_hasher = Sha256::new();
        root_hasher.update(self.current_root);
        root_hasher.update(hash_bytes);
        self.current_root = root_hasher.finalize().into();

        let new_root = hex::encode(self.current_root);

        // Store the proof for later retrieval
        // The proof shows: SHA256(sibling || leaf_hash) = root when is_right = true
        let stored_proof = StoredProof {
            sibling: previous_root.clone(),
            is_right: true, // leaf is always on the right in our incremental scheme
            root: new_root.clone(),
        };
        self.proof_index.insert(hash.clone(), stored_proof);

        // Generate proof for response
        let proof = NomtProof {
            leaf_hash: hash.clone(),
            siblings: vec![previous_root],
            path: vec![true], // true = leaf is on the right (sibling || leaf = parent)
            root: new_root.clone(),
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        };

        // Persist state to disk for durability (async to avoid blocking)
        if let Err(e) = self.persist_async().await {
            warn!(error = %e, "Failed to persist NOMT state to disk");
        }

        Ok(StoreResponse {
            hash,
            nomt_root: new_root,
            proof,
            timestamp,
        })
    }

    /// Verify a hash exists in storage
    /// Reloads from disk first to ensure freshness with shared NFS storage
    pub async fn verify(&mut self, hash: &str) -> anyhow::Result<VerifyResponse> {
        // Reload from disk to get latest state from other replicas (async to avoid blocking)
        if let Err(e) = self.reload_from_disk_async().await {
            warn!(error = %e, "Failed to reload from disk before verify");
        }

        let exists = self.hash_index.contains_key(hash);
        let metadata = self.hash_index.get(hash);

        Ok(VerifyResponse {
            exists,
            verified: exists,
            nomt_root: hex::encode(self.current_root),
            user_id: metadata.map(|m| m.user_id.clone()),
            report_type: metadata.map(|m| m.report_type.clone()),
            timestamp: metadata.map(|m| m.timestamp.clone()),
        })
    }

    /// Get Merkle proof for a hash
    ///
    /// Returns the proof that was stored at insert time. The proof enables
    /// verification by computing: SHA256(sibling || `leaf_hash`) = root
    /// when `path[0] = true` (leaf is on the right).
    /// Reloads from disk first to ensure freshness with shared NFS storage
    pub async fn get_proof(&mut self, hash: &str) -> anyhow::Result<Option<NomtProof>> {
        // Reload from disk to get latest state from other replicas (async to avoid blocking)
        if let Err(e) = self.reload_from_disk_async().await {
            warn!(error = %e, "Failed to reload from disk before get_proof");
        }

        // Check if hash exists
        if !self.hash_index.contains_key(hash) {
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            return Ok(None);
        }

        // Look up the stored proof
        if let Some(stored) = self.proof_index.get(hash) {
            // Return the proof that was generated at insert time
            let proof = NomtProof {
                leaf_hash: hash.to_string(),
                siblings: vec![stored.sibling.clone()],
                path: vec![stored.is_right],
                root: stored.root.clone(),
            };
            return Ok(Some(proof));
        }

        // Fallback for legacy hashes without stored proofs (migrated from old format)
        // These will use self-verifying proof pattern for backward compatibility
        warn!(hash = %hash, "No stored proof found, using legacy self-verifying proof");
        let proof = NomtProof {
            leaf_hash: hash.to_string(),
            siblings: vec![],
            path: vec![],
            root: hash.to_string(), // Legacy: root equals leaf for self-verification
        };

        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        Ok(Some(proof))
    }

    /// Get storage statistics
    /// Reloads from disk first to ensure freshness with shared NFS storage
    /// NOTE: Made async to match `StorageBackend` trait and avoid `block_on` anti-pattern
    pub async fn get_stats(&mut self) -> serde_json::Value {
        // Reload from disk to get latest state from other replicas (async to avoid blocking)
        if let Err(e) = self.reload_from_disk_async().await {
            warn!(error = %e, "Failed to reload from disk before get_stats");
        }

        let unique_users: std::collections::HashSet<_> =
            self.hash_index.values().map(|m| &m.user_id).collect();

        serde_json::json!({
            "total_hashes": self.total_hashes.load(Ordering::Relaxed),
            "total_data_bytes": self.total_bytes.load(Ordering::Relaxed),
            "current_root": hex::encode(self.current_root),
            "unique_users": unique_users.len(),
            "ready": self.ready
        })
    }

    /// Get Prometheus metrics for storage
    /// Reloads from disk first to ensure freshness with shared NFS storage
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    /// NOTE: Made async to match `StorageBackend` trait and avoid `block_on` anti-pattern
    pub async fn get_metrics(&mut self) -> String {
        // Reload from disk to get latest state from other replicas (async to avoid blocking)
        if let Err(e) = self.reload_from_disk_async().await {
            warn!(error = %e, "Failed to reload from disk before get_metrics");
        }

        let unique_users: std::collections::HashSet<_> =
            self.hash_index.values().map(|m| &m.user_id).collect();

        format!(
            "# HELP nomt_total_hashes Total number of stored hashes\n\
             # TYPE nomt_total_hashes counter\n\
             nomt_total_hashes {}\n\
             # HELP nomt_total_bytes Total bytes stored\n\
             # TYPE nomt_total_bytes counter\n\
             nomt_total_bytes {}\n\
             # HELP nomt_unique_users Number of unique users with stored hashes\n\
             # TYPE nomt_unique_users gauge\n\
             nomt_unique_users {}\n\
             # HELP nomt_ready Storage ready status\n\
             # TYPE nomt_ready gauge\n\
             nomt_ready {}\n",
            self.total_hashes.load(Ordering::Relaxed),
            self.total_bytes.load(Ordering::Relaxed),
            unique_users.len(),
            i32::from(self.ready)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_nomt_storage_new_creates_directory() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("nomt_data");
        let _storage = NomtStorage::new(data_dir.to_str().unwrap()).await.unwrap();
        assert!(data_dir.exists());
    }

    #[tokio::test]
    async fn test_nomt_storage_store_returns_hash() {
        let tmp = TempDir::new().unwrap();
        let mut storage = NomtStorage::new(tmp.path().to_str().unwrap())
            .await
            .unwrap();
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        let resp = storage
            .store("user1", "PlaidFinancial", b"report_data")
            .await
            .unwrap();
        assert_eq!(resp.hash.len(), 64);
        assert!(resp.hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn test_nomt_storage_store_increments_stats() {
        let tmp = TempDir::new().unwrap();
        let mut storage = NomtStorage::new(tmp.path().to_str().unwrap())
            .await
            .unwrap();
        for i in 0..3 {
            storage
                .store(
                    &format!("user{i}"),
                    "type",
                    &format!("data{i}").into_bytes(),
                )
                .await
                .unwrap();
        }
        let stats = storage.get_stats().await;
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        assert_eq!(stats["total_hashes"], 3);
    }

    #[tokio::test]
    async fn test_nomt_storage_verify_existing_hash() {
        let tmp = TempDir::new().unwrap();
        let mut storage = NomtStorage::new(tmp.path().to_str().unwrap())
            .await
            .unwrap();
        let resp = storage
            .store("user1", "PlaidFinancial", b"report_data")
            .await
            .unwrap();
        let verify = storage.verify(&resp.hash).await.unwrap();
        assert!(verify.exists);
        assert!(verify.verified);
    }

    #[tokio::test]
    async fn test_nomt_storage_verify_missing_hash() {
        let tmp = TempDir::new().unwrap();
        let mut storage = NomtStorage::new(tmp.path().to_str().unwrap())
            .await
            .unwrap();
        let verify = storage
            .verify("0000000000000000000000000000000000000000000000000000000000000000")
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            .await
            .unwrap();
        assert!(!verify.exists);
        assert!(!verify.verified);
    }

    #[tokio::test]
    async fn test_nomt_storage_get_proof_for_existing() {
        let tmp = TempDir::new().unwrap();
        let mut storage = NomtStorage::new(tmp.path().to_str().unwrap())
            .await
            .unwrap();
        let resp = storage
            .store("user1", "PlaidFinancial", b"report_data")
            .await
            .unwrap();
        let proof = storage.get_proof(&resp.hash).await.unwrap();
        assert!(proof.is_some());
        let proof = proof.unwrap();
        assert_eq!(proof.leaf_hash, resp.hash);
        assert_eq!(proof.root, resp.nomt_root);
    }

    #[tokio::test]
    async fn test_nomt_storage_get_proof_for_missing() {
        let tmp = TempDir::new().unwrap();
        let mut storage = NomtStorage::new(tmp.path().to_str().unwrap())
            .await
            .unwrap();
        let proof = storage
            .get_proof("0000000000000000000000000000000000000000000000000000000000000000")
            .await
            .unwrap();
        assert!(proof.is_none());
    }

    #[tokio::test]
    async fn test_nomt_storage_root_changes_after_store() {
        let tmp = TempDir::new().unwrap();
        let mut storage = NomtStorage::new(tmp.path().to_str().unwrap())
            .await
            .unwrap();
        let root_before = storage.current_root();
        storage
            .store("user1", "PlaidFinancial", b"report_data")
            .await
            .unwrap();
        let root_after = storage.current_root();
        assert_ne!(root_before, root_after);
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001

    #[tokio::test]
    async fn test_nomt_storage_is_ready() {
        let tmp = TempDir::new().unwrap();
        let storage = NomtStorage::new(tmp.path().to_str().unwrap())
            .await
            .unwrap();
        assert!(storage.is_ready());
    }

    #[tokio::test]
    async fn test_nomt_storage_current_root() {
        let tmp = TempDir::new().unwrap();
        let storage = NomtStorage::new(tmp.path().to_str().unwrap())
            .await
            .unwrap();
        let root = storage.current_root();
        assert_eq!(root.len(), 64);
        assert_eq!(root, "0".repeat(64));
    }

    #[tokio::test]
    async fn test_nomt_storage_persist_and_reload() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        let mut storage = NomtStorage::new(path).await.unwrap();
        let resp = storage
            .store("user1", "PlaidFinancial", b"report_data")
            .await
            .unwrap();
        let hash = resp.hash;
        drop(storage);

        let mut storage2 = NomtStorage::new(path).await.unwrap();
        let verify = storage2.verify(&hash).await.unwrap();
        assert!(verify.exists);
    }

    #[tokio::test]
    async fn test_nomt_storage_get_stats_unique_users() {
        let tmp = TempDir::new().unwrap();
        let mut storage = NomtStorage::new(tmp.path().to_str().unwrap())
            .await
            .unwrap();
        storage.store("user_a", "type", b"data1").await.unwrap();
        storage.store("user_a", "type", b"data2").await.unwrap();
        storage.store("user_b", "type", b"data3").await.unwrap();
        let stats = storage.get_stats().await;
        assert_eq!(stats["unique_users"], 2);
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
