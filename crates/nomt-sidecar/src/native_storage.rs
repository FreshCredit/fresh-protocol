// TAG: surface=blockchain owner=blockchain-team rule=BC-001
//! Native NOMT Storage Implementation (current)
//!
//! This module provides native NOMT storage using the real nomt library
//! with Beatree + Bitbox for page-aligned storage on `NVMe` SSDs.
//!
//! Key features:
//! - Uses `nomt::Nomt<Blake3Hasher>` for the merkle trie database
//! - Page-aligned I/O for optimal `NVMe` performance
//! - `io_uring` on Linux for async I/O
//! - Generates proper Merkle witnesses/proofs
//!
//! Requirements:
//! - Linux with kernel 6.x+ for `io_uring`
//! - Local disk storage (not NFS - `io_uring` doesn't work with NFS)
//! - `NOMT_NATIVE=true` environment variable

use crate::types::{HashMetadata, NomtProof, StoreResponse, VerifyResponse};
use nomt::{
    hasher::Blake3Hasher, trie::KeyPath, KeyReadWrite, Nomt, Options, SessionParams, WitnessMode,
};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Native NOMT Storage using the real nomt library
pub struct NativeNomtStorage {
    /// The NOMT database instance
    nomt: Arc<Nomt<Blake3Hasher>>,
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    /// Data directory
    data_dir: PathBuf,
    /// Ordered metadata index (BTreeMap for deterministic RFC 6962 proof synthesis)
    metadata: RwLock<BTreeMap<String, HashMetadata>>,
    /// Secondary index mapping SHA-256(data) -> bound leaf hash
    sha256_aliases: RwLock<HashMap<String, String>>,
    /// Latest RFC 6962 Merkle root aligned with `freshcredit-nomt-core`
    rfc6962_root: RwLock<String>,
    /// Ready flag
    ready: bool,
}

impl std::fmt::Debug for NativeNomtStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeNomtStorage")
            .field("data_dir", &self.data_dir)
            .field("ready", &self.ready)
            .finish_non_exhaustive()
    }
}

impl NativeNomtStorage {
    /// Create a new native NOMT storage instance
    pub async fn new(data_dir: &str) -> anyhow::Result<Self> {
        let path = PathBuf::from(data_dir);
        let nomt_path = path.join("nomt_native");

        // Create directory if needed
        // P1-PERF: Wrap blocking I/O in spawn_blocking
        if !nomt_path.exists() {
            let path_clone = nomt_path.clone();
            tokio::task::spawn_blocking(move || std::fs::create_dir_all(&path_clone)).await??;
        }

        info!(path = %nomt_path.display(), "Initializing native NOMT storage");
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001

        // Configure NOMT options
        let mut opts = Options::new();
        opts.path(
            nomt_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid path"))?,
        );
        opts.commit_concurrency(4); // Allow concurrent commits
        opts.io_workers(4); // I/O worker threads

        // Open NOMT database
        let nomt = Nomt::<Blake3Hasher>::open(opts)?;

        info!(
            root = %hex::encode(nomt.root().into_inner()),
            empty = nomt.is_empty(),
            "Native NOMT database opened"
        );

        // Load metadata index if it exists
        // P1-PERF: Wrap blocking I/O in spawn_blocking
        let metadata_file = path.join("metadata_index.json");
        let metadata: BTreeMap<String, HashMetadata> = if metadata_file.exists() {
            let file_clone = metadata_file.clone();
            (tokio::task::spawn_blocking(move || std::fs::read_to_string(&file_clone)).await?)
                .map_or_else(
                    |_| BTreeMap::new(),
                    |content| serde_json::from_str(&content).unwrap_or_default(),
                )
        } else {
            BTreeMap::new()
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        };
        let ordered_init: Vec<String> = metadata.keys().cloned().collect();
        let initial_root = if ordered_init.is_empty() {
            hex::encode(nomt.root().into_inner())
        } else {
            freshcredit_nomt_core::compute_merkle_root(&ordered_init)
        };

        Ok(Self {
            nomt: Arc::new(nomt),
            data_dir: path,
            metadata: RwLock::new(metadata),
            sha256_aliases: RwLock::new(HashMap::new()),
            rfc6962_root: RwLock::new(initial_root),
            ready: true,
        })
    }

    /// Persist metadata index to disk
    async fn persist_metadata(&self) -> anyhow::Result<()> {
        let metadata = self.metadata.read().await;
        let content = serde_json::to_string_pretty(&*metadata)?;
        let metadata_file = self.data_dir.join("metadata_index.json");
        // P1-PERF: Wrap blocking I/O in spawn_blocking
        let file_clone = metadata_file.clone();
        tokio::task::spawn_blocking(move || std::fs::write(&file_clone, content)).await??;
        Ok(())
    }

    /// Convert a hex hash to a `KeyPath` (256-bit key)
    fn hash_to_keypath(hash: &str) -> anyhow::Result<KeyPath> {
        let bytes = hex::decode(hash)?;
        if bytes.len() != 32 {
            anyhow::bail!("Hash must be 32 bytes");
        }
        Ok(bytes)
    }

    /// Check if storage is ready
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    pub const fn is_ready(&self) -> bool {
        self.ready
    }

    /// Get current root hash
    pub fn current_root(&self) -> String {
        hex::encode(self.nomt.root().into_inner())
    }

    /// Store report data and generate Merkle proof
    pub async fn store(
        &self,
        user_id: &str,
        report_type: &str,
        data: &[u8],
    ) -> anyhow::Result<StoreResponse> {
        // Compute key-bound RFC 6962 BLAKE3 leaf digest matching LegacyStorage
        let namespace = format!("{user_id}:{report_type}");
        let hash = freshcredit_nomt_core::compute_bound_leaf_hash(&namespace, data);
        let hash_bytes_vec = hex::decode(&hash)?;
        let hash_bytes: [u8; 32] = hash_bytes_vec
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid leaf digest length"))?;

        debug!(hash = %hash, report_type = %report_type, "Storing data in native NOMT");

        let key_path = hash_bytes.to_vec(); // Use bound digest as 256-bit KeyPath

        // Begin a session with witness generation
        let session = self
            .nomt
            .begin_session(SessionParams::default().witness_mode(WitnessMode::read_write()));
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001

        // Warm up the key path for writing
        session.warm_up(key_path.clone());

        // Prepare the value (store the raw data)
        let value = data.to_vec();

        // Finish the session with the write operation
        let actual_access = vec![(key_path.clone(), KeyReadWrite::Write(Some(value)))];

        let mut finished = session.finish(actual_access)?;
        let _nomt_trie_root = finished.root();
        let witness = finished.take_witness();

        // Commit the changes to Beatree + Bitbox
        finished.commit(&self.nomt)?;

        // Fail closed if native NOMT witness generation failed
        let _w = witness.ok_or_else(|| {
            anyhow::anyhow!("Native NOMT failed to emit cryptographic witness proof")
        })?;

        let sha256_alias = {
            use sha2::{Digest, Sha256};
            hex::encode(Sha256::digest(data))
        };

        // Store metadata in BTreeMap & construct canonical RFC 6962 BLAKE3 proof matching LegacyStorage
        let timestamp = chrono::Utc::now().to_rfc3339();
        let metadata = HashMetadata {
            user_id: user_id.to_string(),
            report_type: report_type.to_string(),
            timestamp: timestamp.clone(),
            data_size: data.len(),
        };

        let (ordered_leaves, new_root_hex) = {
            let mut meta_guard = self.metadata.write().await;
            meta_guard.insert(hash.clone(), metadata);
            let leaves: Vec<String> = meta_guard.keys().cloned().collect();
            let root = freshcredit_nomt_core::compute_merkle_root(&leaves);
            (leaves, root)
        };
        {
            self.sha256_aliases
                .write()
                .await
                .entry(sha256_alias)
                .or_insert_with(|| hash.clone());
            *self.rfc6962_root.write().await = new_root_hex.clone();
        }

        let core_proof = freshcredit_nomt_core::build_merkle_proof(&ordered_leaves, &hash)
            .ok_or_else(|| anyhow::anyhow!("Failed to construct RFC 6962 Merkle proof"))?;
        let proof = NomtProof {
            leaf_hash: core_proof.leaf_hash,
            siblings: core_proof.siblings,
            path: core_proof.path,
            root: core_proof.root,
        };

        // Persist metadata to disk
        self.persist_metadata().await?;

        info!(
            hash = %hash,
            new_root = %new_root_hex,
            "Data stored in native NOMT"
        );

        Ok(StoreResponse {
            hash,
            nomt_root: new_root_hex,
            proof,
            timestamp,
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        })
    }

    /// Resolve either a bound leaf hash or a registered on-chain SHA-256 alias
    async fn resolve_leaf_key(&self, hash_or_sha256: &str) -> Option<String> {
        if self.metadata.read().await.contains_key(hash_or_sha256) {
            return Some(hash_or_sha256.to_string());
        }
        let alias = self
            .sha256_aliases
            .read()
            .await
            .get(hash_or_sha256)
            .cloned();
        if let Some(ref leaf) = alias {
            if self.metadata.read().await.contains_key(leaf) {
                return alias;
            }
        }
        None
    }

    /// Generate an RFC 6962 inclusion proof on demand for native storage
    pub async fn get_proof(&self, hash: &str) -> anyhow::Result<Option<NomtProof>> {
        let Some(leaf_key) = self.resolve_leaf_key(hash).await else {
            return Ok(None);
        };
        let ordered_leaves: Vec<String> = self.metadata.read().await.keys().cloned().collect();
        Ok(
            freshcredit_nomt_core::build_merkle_proof(&ordered_leaves, &leaf_key).map(|p| {
                NomtProof {
                    leaf_hash: p.leaf_hash,
                    siblings: p.siblings,
                    path: p.path,
                    root: p.root,
                }
            }),
        )
    }

    /// Verify a hash exists in storage
    pub async fn verify(
        &self,
        hash: &str,
        expected_root: Option<&str>,
    ) -> anyhow::Result<VerifyResponse> {
        let resolved = self.resolve_leaf_key(hash).await;
        let lookup_key = resolved.as_deref().unwrap_or(hash);
        let key_path = Self::hash_to_keypath(lookup_key)?;

        // Begin a read session
        let session = self.nomt.begin_session(SessionParams::default());

        // Read the value (ENOMT API: read expects KeyPath which is Vec<u8>)
        let value = session.read(key_path)?;
        let exists = value.is_some();

        // Finish the session (no writes)
        let _finished = session.finish(vec![])?;
        let current_root = self.rfc6962_root.read().await.clone();

        // Check root if expected
        let verified = expected_root.map_or(exists, |expected| exists && current_root == expected);

        // Get metadata
        let metadata = self.metadata.read().await;
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        let meta = metadata.get(lookup_key);

        Ok(VerifyResponse {
            exists,
            verified,
            nomt_root: current_root,
            user_id: meta.map(|m| m.user_id.clone()),
            report_type: meta.map(|m| m.report_type.clone()),
            timestamp: meta.map(|m| m.timestamp.clone()),
        })
    }

    /// Get storage statistics
    pub async fn stats(&self) -> crate::types::StorageStats {
        let metadata = self.metadata.read().await;
        let total_hashes = metadata.len() as u64;
        let total_bytes: u64 = metadata.values().map(|m| m.data_size as u64).sum();
        let unique_users = metadata
            .values()
            .map(|m| m.user_id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len() as u64;

        crate::types::StorageStats {
            total_hashes,
            total_data_bytes: total_bytes,
            current_root: self.current_root(),
            unique_users,
            efficiency_ratio: if total_bytes > 0 { 1.0 } else { 0.0 },
        }
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
