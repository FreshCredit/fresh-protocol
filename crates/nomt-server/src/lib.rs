// TAG: surface=blockchain owner=blockchain-team rule=BC-001
//! NOMT Server - Storage implementations for NOMT proofs
//!
//! This crate provides server-side storage implementations for the NOMT system,
//! unifying the legacy JSON-based storage and native NOMT library storage.

#![warn(missing_docs)]
#![deny(unsafe_code)]

use async_trait::async_trait;
use freshcredit_nomt_core::{
    ProofError, StorageStats, StoreRequest, StoreResponse, VerifyRequest, VerifyResponse,
};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

pub mod storage;

#[cfg(feature = "native")]
pub use storage::NativeStorage;
pub use storage::{LegacyStorage, StorageBackend};

/// Errors that can occur in NOMT server operations
#[derive(Debug, Error)]
pub enum ServerError {
    /// Storage error
    #[error("Storage error: {0}")]
    Storage(String),
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    /// Proof error
    #[error(transparent)]
    Proof(#[from] ProofError),
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// Feature not implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

/// Unified storage trait for NOMT operations
#[async_trait]
pub trait Storage: Send + Sync {
    /// Store data and return the proof
    async fn store(&self, request: StoreRequest) -> Result<StoreResponse, ServerError>;

    /// Verify a hash and return the proof
    async fn verify(&self, request: VerifyRequest) -> Result<VerifyResponse, ServerError>;

    /// Get current storage statistics
    async fn stats(&self) -> Result<StorageStats, ServerError>;

    /// Get the current Merkle root
    async fn current_root(&self) -> Result<String, ServerError>;
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}

/// Unified storage that can use either legacy or native backend
pub struct UnifiedStorage {
    backend: Arc<RwLock<dyn Storage>>,
}

impl std::fmt::Debug for UnifiedStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedStorage").finish_non_exhaustive()
    }
}

impl UnifiedStorage {
    /// Create a new unified storage with a specific backend
    pub fn new(backend: Arc<RwLock<dyn Storage>>) -> Self {
        Self { backend }
    }

    /// Create with legacy storage backend
    #[cfg(feature = "legacy")]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn legacy(path: impl AsRef<std::path::Path>) -> Result<Self, ServerError> {
        let storage = LegacyStorage::new(path)?;
        Ok(Self::new(Arc::new(RwLock::new(storage))))
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001

    /// Create with native NOMT storage backend
    #[cfg(feature = "native")]
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn native(path: impl AsRef<std::path::Path>) -> Result<Self, ServerError> {
        let storage = NativeStorage::new(path)?;
        Ok(Self::new(Arc::new(RwLock::new(storage))))
    }

    /// Store data
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn store(&self, request: StoreRequest) -> Result<StoreResponse, ServerError> {
        let backend = self.backend.read().await;
        backend.store(request).await
    }

    /// Verify hash
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn verify(&self, request: VerifyRequest) -> Result<VerifyResponse, ServerError> {
        let backend = self.backend.read().await;
        backend.verify(request).await
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    /// Get stats
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn stats(&self) -> Result<StorageStats, ServerError> {
        let backend = self.backend.read().await;
        backend.stats().await
    }

    /// Get current root
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn current_root(&self) -> Result<String, ServerError> {
        let backend = self.backend.read().await;
        backend.current_root().await
    }
}

/// Type alias for thread-safe storage
pub type SharedStorage = Arc<RwLock<dyn Storage>>;

/// Create a new shared storage instance
pub fn shared_storage<S: Storage + 'static>(storage: S) -> SharedStorage {
    Arc::new(RwLock::new(storage))
}

#[cfg(test)]
// TAG: surface=blockchain owner=blockchain-team rule=BC-001
mod tests {
    use super::*;
    use freshcredit_nomt_core::HashMetadata;

    #[test]
    fn test_server_error_display_storage() {
        let err = ServerError::Storage("test error".to_string());
        assert_eq!(err.to_string(), "Storage error: test error");
    }

    #[test]
    fn test_server_error_display_not_implemented() {
        let err = ServerError::NotImplemented("feature X".to_string());
        assert_eq!(err.to_string(), "Not implemented: feature X");
    }

    #[test]
    fn test_server_error_from_proof_error() {
        let proof_err = ProofError::InvalidHash("bad".to_string());
        let err: ServerError = proof_err.into();
        assert!(matches!(err, ServerError::Proof(_)));
    }

    #[test]
    fn test_server_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: ServerError = io_err.into();
        assert!(matches!(err, ServerError::Io(_)));
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        assert!(err.to_string().contains("file missing"));
    }

    #[test]
    fn test_server_error_from_serde_error() {
        let serde_err = serde_json::Error::io(std::io::Error::other("bad json"));
        let err: ServerError = serde_err.into();
        assert!(matches!(err, ServerError::Serialization(_)));
    }

    #[test]
    fn test_unified_storage_new() {
        let storage = LegacyStorage::new("/tmp/test_nomt").unwrap();
        let unified = UnifiedStorage::new(shared_storage(storage));
        // Just verify it compiles and creates
        assert!(std::mem::size_of_val(&unified) > 0);
    }

    #[tokio::test]
    async fn test_unified_storage_delegate_methods() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = LegacyStorage::new(tmp.path()).unwrap();
        let unified = UnifiedStorage::new(shared_storage(legacy));

        // store
        let req = StoreRequest {
            data: "hello".to_string(),
            metadata: Some(HashMetadata::new("test", "id-1")),
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        };
        let resp = unified.store(req.clone()).await.unwrap();
        assert!(!resp.hash.is_empty());

        // verify
        let verify_req = VerifyRequest {
            hash: resp.hash.clone(),
            expected_root: None,
        };
        let verify_resp = unified.verify(verify_req).await.unwrap();
        assert!(verify_resp.verified);

        // stats
        let stats = unified.stats().await.unwrap();
        assert_eq!(stats.total_items, 1);
        assert_eq!(stats.backend_type, "legacy");

        // current_root
        let root = unified.current_root().await.unwrap();
        assert!(!root.is_empty());
    }

    #[test]
    fn test_shared_storage() {
        let storage = LegacyStorage::new("/tmp/test_shared").unwrap();
        let shared: SharedStorage = shared_storage(storage);
        assert!(std::mem::size_of_val(&shared) > 0);
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
