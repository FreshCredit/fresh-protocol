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
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
// fs2::FileExt used via fully qualified syntax for NFS file locking

/// Persistent state for NOMT storage (deterministic BTreeMap JSON mode)
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct PersistentState {
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    /// Deterministic ordered hash index with metadata
    hash_index: BTreeMap<String, HashMetadata>,
    /// Secondary index mapping on-chain SHA-256 report digests -> bound RFC 6962 leaf hashes
    #[serde(default)]
    sha256_aliases: BTreeMap<String, String>,
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
/// Builds a deterministic RFC 6962 BLAKE3 binary Merkle tree across all stored
/// leaves so every historical item verifies against the current anchored root.
#[derive(Debug)]
pub struct NomtStorage {
    /// Data directory for NOMT storage
    data_dir: PathBuf,
    /// Deterministic ordered index of stored leaves (keyed by bound leaf hash)
    hash_index: BTreeMap<String, HashMetadata>,
    /// Secondary index mapping SHA-256(data) -> bound leaf hash for on-chain hash lookup
    sha256_aliases: BTreeMap<String, String>,
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
