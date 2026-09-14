// TAG: surface=blockchain owner=blockchain-team rule=BC-001
//! NOMT Storage Implementation
//!
//! This module provides the storage layer for the NOMT sidecar.
//! It wraps the NOMT library and provides a high-level API for storing
//! and verifying report data with Merkle proofs.
//!
//! current Architecture:
//! - **Native NOMT mode** (`NOMT_NATIVE=true)`: Uses the real NOMT library with
//!   Beatree + Bitbox for page-aligned storage. Optimized for `NVMe` SSDs with `io_uring`.
//! - **Legacy JSON mode** (`NOMT_NATIVE=false)`: Uses HashMap+JSON with NFS file locking.
//!   Works with shared Filestore storage for multi-replica deployments.
//!
//! The storage backend is selected at runtime based on `NOMT_NATIVE` environment variable.

use crate::types::{HashMetadata, NomtProof, StoreResponse, VerifyResponse};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
// fs2::FileExt used via fully qualified syntax for NFS file locking

/// Stored proof data for a hash (generated at insert time)
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
struct StoredProof {
    /// The sibling hash (previous root for incremental tree)
    sibling: String,
    /// Path bit (true = hash is on right, false = hash is on left)
    is_right: bool,
    /// Root after this hash was inserted
    root: String,
}

/// Persistent state for NOMT storage (legacy JSON mode)
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct PersistentState {
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    /// Hash index with metadata
    hash_index: HashMap<String, HashMetadata>,
    /// Proof index: hash -> proof generated at insert time
    proof_index: HashMap<String, StoredProof>,
    /// Current root hash (hex encoded)
    current_root: String,
    /// Total hashes stored
    total_hashes: u64,
    /// Total bytes stored
    total_bytes: u64,
}

/// NOMT Storage wrapper
///
/// Provides high-level operations for storing and verifying report data.
/// Uses NOMT's binary Merkle trie for efficient proof generation.
/// Persists state to disk for production reliability.

#[derive(Debug)]
pub struct NomtStorage {
    /// Data directory for NOMT storage
    data_dir: PathBuf,
    /// In-memory index of stored hashes (for quick lookups)
    hash_index: HashMap<String, HashMetadata>,
    /// Proof index: stores the proof generated at insert time for each hash
    proof_index: HashMap<String, StoredProof>,
    /// Current NOMT root hash
    current_root: [u8; 32],
    /// Storage statistics
    total_hashes: AtomicU64,
    total_bytes: AtomicU64,
    /// Ready flag
    ready: bool,
}

// TAG: surface=blockchain owner=blockchain-team rule=BC-001
pub mod impls;
