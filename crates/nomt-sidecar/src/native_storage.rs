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
use sha2::{Digest, Sha256};
use std::collections::HashMap;
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
    /// Metadata index (stored separately since NOMT only stores key-value pairs)
    metadata: RwLock<HashMap<String, HashMetadata>>,
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
        let metadata = if metadata_file.exists() {
            let file_clone = metadata_file.clone();
            (tokio::task::spawn_blocking(move || std::fs::read_to_string(&file_clone)).await?)
                .map_or_else(
                    |_| HashMap::new(),
                    |content| serde_json::from_str(&content).unwrap_or_default(),
                )
        } else {
            HashMap::new()
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        };

        Ok(Self {
            nomt: Arc::new(nomt),
            data_dir: path,
            metadata: RwLock::new(metadata),
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
        // Compute SHA-256 hash of the data
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash_bytes: [u8; 32] = hasher.finalize().into();
        let hash = hex::encode(hash_bytes);

        debug!(hash = %hash, report_type = %report_type, "Storing data in native NOMT");

        let key_path = hash_bytes.to_vec(); // Use hash as key (KeyPath is Vec<u8>)
        let prev_root = self.nomt.root();

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
        let new_root = finished.root();
        let witness = finished.take_witness();

        // Commit the changes
        finished.commit(&self.nomt)?;

        // Generate proof from witness
        let proof = if let Some(w) = witness {
            // Extract proof data from witness - sibling_chunks are the sibling nodes along the path
            // ENOMT API changed: siblings -> sibling_chunks (Vec<SiblingChunk>)
            let siblings: Vec<String> = w
                .path_proofs
                .iter()
                .flat_map(|pp| {
                    // Get sibling hashes from the path proof
                    pp.inner
                        .sibling_chunks
                        .iter()
                        .map(|chunk| {
                            // SiblingChunk is an enum with Sibling(Node) and Terminators(u16) variants
                            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
                            match chunk {
                                nomt::proof::SiblingChunk::Sibling(node) => hex::encode(node),
                                nomt::proof::SiblingChunk::Terminators(_) => {
                                    // Terminator chunks don't have a hash, use zero hash
                                    hex::encode([0u8; 32])
                                }
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect();

            // Path bits derived from the key hash
            let path_bits: Vec<bool> = (0..siblings.len())
                .map(|i| (hash_bytes[i / 8] >> (7 - (i % 8))) & 1 == 1)
                .collect();

            NomtProof {
                leaf_hash: hash.clone(),
                siblings,
                path: path_bits,
                root: hex::encode(new_root.into_inner()),
            }
        } else {
            // Fallback proof if witness not available
            NomtProof {
                leaf_hash: hash.clone(),
                siblings: vec![hex::encode(prev_root.into_inner())],
                path: vec![true],
                root: hex::encode(new_root.into_inner()),
            }
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        };

        // Store metadata
        let timestamp = chrono::Utc::now().to_rfc3339();
        let metadata = HashMetadata {
            user_id: user_id.to_string(),
            report_type: report_type.to_string(),
            timestamp: timestamp.clone(),
            data_size: data.len(),
        };

        {
            let mut meta_guard = self.metadata.write().await;
            meta_guard.insert(hash.clone(), metadata);
        }

        // Persist metadata to disk
        self.persist_metadata().await?;

        info!(
            hash = %hash,
            prev_root = %hex::encode(prev_root.into_inner()),
            new_root = %hex::encode(new_root.into_inner()),
            "Data stored in native NOMT"
        );

        Ok(StoreResponse {
            hash,
            nomt_root: hex::encode(new_root.into_inner()),
            proof,
            timestamp,
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        })
    }

    /// Verify a hash exists in storage
    pub async fn verify(
        &self,
        hash: &str,
        expected_root: Option<&str>,
    ) -> anyhow::Result<VerifyResponse> {
        let key_path = Self::hash_to_keypath(hash)?;

        // Begin a read session
        let session = self.nomt.begin_session(SessionParams::default());

        // Read the value (ENOMT API: read expects KeyPath which is Vec<u8>)
        let value = session.read(key_path.clone())?;
        let exists = value.is_some();

        // Finish the session (no writes)
        let finished = session.finish(vec![])?;
        let current_root = hex::encode(finished.root().into_inner());

        // Check root if expected
        let verified = expected_root.map_or(exists, |expected| exists && current_root == expected);

        // Get metadata
        let metadata = self.metadata.read().await;
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        let meta = metadata.get(hash);

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
