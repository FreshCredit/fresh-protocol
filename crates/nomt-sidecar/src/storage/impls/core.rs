// TAG: surface=blockchain owner=blockchain-team rule=BC-001
use crate::storage::*;
use std::collections::BTreeMap;
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
        let (hash_index, sha256_aliases, current_root, total_hashes, total_bytes) = if state_file
            .exists()
        {
            let state_file_clone = state_file.clone();
            match tokio::task::spawn_blocking(move || std::fs::read_to_string(&state_file_clone))
                .await?
            {
                Ok(content) => match serde_json::from_str::<PersistentState>(&content) {
                    Ok(state) => {
                        let root_hex = if state.hash_index.is_empty() {
                            state.current_root.clone()
                        } else {
                            let ordered: Vec<String> = state.hash_index.keys().cloned().collect();
                            freshcredit_nomt_core::compute_merkle_root(&ordered)
                        };
                        let root_bytes = hex::decode(&root_hex)
                            .ok()
                            .and_then(|b| <[u8; 32]>::try_from(b).ok())
                            .unwrap_or([0u8; 32]);
                        info!(
                            path = %state_file.display(),
                            hashes = state.total_hashes,
                            "Loaded existing NOMT state from disk"
                        );
                        (
                            state.hash_index,
                            state.sha256_aliases,
                            root_bytes,
                            state.total_hashes,
                            state.total_bytes,
                        )
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to parse NOMT state, starting fresh");
                        (BTreeMap::new(), BTreeMap::new(), [0u8; 32], 0, 0)
                    }
                },
                Err(e) => {
                    warn!(error = %e, "Failed to read NOMT state file, starting fresh");
                    (BTreeMap::new(), BTreeMap::new(), [0u8; 32], 0, 0)
                }
            }
        } else {
            info!(path = %path.display(), "No existing NOMT state, starting fresh");
            (BTreeMap::new(), BTreeMap::new(), [0u8; 32], 0, 0)
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
            sha256_aliases,
            current_root,
            total_hashes: AtomicU64::new(total_hashes),
            total_bytes: AtomicU64::new(total_bytes),
            ready: true,
        })
    }

    /// Store report data and generate an RFC 6962 BLAKE3 Merkle inclusion proof
    /// against the updated global `current_root`.
    ///
    /// Leaf preimage binds `(user_id, report_type)` to `data` via
    /// `compute_bound_leaf_hash` so proofs cannot be transplanted across identities.
    pub async fn store(
        &mut self,
        user_id: &str,
        report_type: &str,
        data: &[u8],
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    ) -> anyhow::Result<StoreResponse> {
        let namespace = format!("{user_id}:{report_type}");
        let hash = freshcredit_nomt_core::compute_bound_leaf_hash(&namespace, data);
        let sha256_alias = {
            use sha2::{Digest, Sha256};
            hex::encode(Sha256::digest(data))
        };

        debug!(hash = %hash, report_type = %report_type, "Storing report data");

        let timestamp = chrono::Utc::now().to_rfc3339();
        let metadata = HashMetadata {
            user_id: user_id.to_string(),
            report_type: report_type.to_string(),
            timestamp: timestamp.clone(),
            data_size: data.len(),
        };
        let inserted_new = self.hash_index.insert(hash.clone(), metadata).is_none();
        self.sha256_aliases
            .entry(sha256_alias)
            .or_insert_with(|| hash.clone());
        if inserted_new {
            self.total_hashes.fetch_add(1, Ordering::Relaxed);
            self.total_bytes
                .fetch_add(data.len() as u64, Ordering::Relaxed);
        }

        let ordered_leaves: Vec<String> = self.hash_index.keys().cloned().collect();
        let new_root = freshcredit_nomt_core::compute_merkle_root(&ordered_leaves);
        if let Ok(decoded) = hex::decode(&new_root) {
            if let Ok(arr) = <[u8; 32]>::try_from(decoded) {
                self.current_root = arr;
            }
        }

        let core_proof = freshcredit_nomt_core::build_merkle_proof(&ordered_leaves, &hash)
            .ok_or_else(|| anyhow::anyhow!("Failed to construct Merkle proof for stored leaf"))?;
        let proof = NomtProof {
            leaf_hash: core_proof.leaf_hash,
            siblings: core_proof.siblings,
            path: core_proof.path,
            root: core_proof.root,
        };

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

    fn resolve_leaf_key<'a>(&'a self, hash_or_sha256: &'a str) -> Option<&'a str> {
        if self.hash_index.contains_key(hash_or_sha256) {
            Some(hash_or_sha256)
        } else {
            self.sha256_aliases
                .get(hash_or_sha256)
                .map(String::as_str)
                .filter(|leaf| self.hash_index.contains_key(*leaf))
        }
    }

    /// Verify a hash exists in storage (accepts bound leaf hash or on-chain SHA-256 report digest)
    /// Reloads from disk first to ensure freshness with shared NFS storage
    pub async fn verify(&mut self, hash: &str) -> anyhow::Result<VerifyResponse> {
        // Reload from disk to get latest state from other replicas (async to avoid blocking)
        if let Err(e) = self.reload_from_disk_async().await {
            warn!(error = %e, "Failed to reload from disk before verify");
        }

        let resolved = self.resolve_leaf_key(hash).map(str::to_string);
        let exists = resolved.is_some();
        let metadata = resolved.as_deref().and_then(|k| self.hash_index.get(k));

        Ok(VerifyResponse {
            exists,
            verified: exists,
            nomt_root: hex::encode(self.current_root),
            user_id: metadata.map(|m| m.user_id.clone()),
            report_type: metadata.map(|m| m.report_type.clone()),
            timestamp: metadata.map(|m| m.timestamp.clone()),
        })
    }

    /// Get Merkle proof for a hash (bound leaf hash or on-chain SHA-256 alias) against the live `current_root`.
    /// Reloads from disk first to ensure freshness with shared NFS storage.
    pub async fn get_proof(&mut self, hash: &str) -> anyhow::Result<Option<NomtProof>> {
        // Reload from disk to get latest state from other replicas (async to avoid blocking)
        if let Err(e) = self.reload_from_disk_async().await {
            warn!(error = %e, "Failed to reload from disk before get_proof");
        }

        let Some(leaf_key) = self.resolve_leaf_key(hash).map(str::to_string) else {
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            return Ok(None);
        };

        let ordered_leaves: Vec<String> = self.hash_index.keys().cloned().collect();
        let Some(core_proof) =
            freshcredit_nomt_core::build_merkle_proof(&ordered_leaves, &leaf_key)
        else {
            return Ok(None);
        };

        Ok(Some(NomtProof {
            leaf_hash: core_proof.leaf_hash,
            siblings: core_proof.siblings,
            path: core_proof.path,
            root: core_proof.root,
        }))
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
        assert!(
            resp.proof.verify(&resp.nomt_root),
            "Sidecar proof must pass canonical RFC 6962 NomtProof::verify()"
        );
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
    async fn test_historical_leaves_verify_against_latest_root_and_bind_keys() {
        let tmp = TempDir::new().unwrap();
        let mut storage = NomtStorage::new(tmp.path().to_str().unwrap())
            .await
            .unwrap();
        let mut hashes = Vec::new();
        for i in 0..5 {
            let resp = storage
                .store(&format!("user-{i}"), "CreditReport", b"identical_payload")
                .await
                .unwrap();
            hashes.push(resp.hash);
        }
        // Key-binding guarantees identical payloads under different users yield distinct leaves
        let unique: std::collections::HashSet<_> = hashes.iter().collect();
        assert_eq!(unique.len(), 5);

        let latest_root = storage.current_root();
        for h in &hashes {
            let proof = storage.get_proof(h).await.unwrap().expect("proof exists");
            assert_eq!(proof.root, latest_root);
            assert!(
                proof.verify_against(&latest_root, h),
                "Historical leaf {h} must verify against latest root {latest_root}"
            );
            assert!(
                !proof.verify_against(&latest_root, &hashes[0]) || h == &hashes[0],
                "Cross-key proof substitution must fail"
            );
        }
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

    #[tokio::test]
    async fn test_on_chain_sha256_alias_resolves_proof_and_verify() {
        use sha2::{Digest, Sha256};

        let tmp = TempDir::new().unwrap();
        let mut storage = NomtStorage::new(tmp.path().to_str().unwrap())
            .await
            .unwrap();
        let payload = br#"{"credit_score":742}"#;
        let resp = storage
            .store("user-alias", "CreditReport", payload)
            .await
            .unwrap();
        let sha256_hex = hex::encode(Sha256::digest(payload));
        assert_ne!(sha256_hex, resp.hash);

        let verified = storage.verify(&sha256_hex).await.unwrap();
        assert!(verified.exists && verified.verified);

        let proof = storage
            .get_proof(&sha256_hex)
            .await
            .unwrap()
            .expect("on-chain SHA-256 alias must resolve Merkle proof");
        assert!(proof.verify_against(&verified.nomt_root, &resp.hash));
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
