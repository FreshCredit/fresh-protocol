// TAG: surface=blockchain owner=blockchain-team rule=BC-001
//! NOMT Sidecar Service for `FreshCredit` Blockchain
//!
//! This service provides high-performance Merkle trie storage for report data,
//! running alongside the Substrate blockchain node. It stores full report data
//! off-chain while only minimal anchor hashes are stored on-chain.
//!
//! Architecture:
//! - Substrate Node: On-chain consensus, finality, anchor hashes
//! - NOMT Sidecar: Off-chain state, Merkle proofs, fast queries
//!
//! Compliance Note: This service stores report data for integrity verification.
//! It does NOT perform scoring, underwriting, or financial decision-making.

#![allow(clippy::wildcard_imports)]
#![allow(clippy::significant_drop_tightening)]

use axum::{
    routing::{get, post},
    Router,
};

use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, Level};

// TAG: surface=blockchain owner=blockchain-team rule=BC-001
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod anchoring;
mod handlers;
mod native_storage;
mod peer_sync;
mod storage;
mod types;

use anchoring::{AnchoringConfig, AnchoringService};
use handlers::*;
use native_storage::NativeNomtStorage;
use peer_sync::{PeerSyncConfig, PeerSyncService};
use storage::NomtStorage;
use types::{NomtProof, StoreResponse, VerifyResponse};

/// CORS layer configuration - environment-aware
/// P1-SECURITY: Restricted origins to prevent CSRF attacks
fn cors_layer() -> CorsLayer {
    use tower_http::cors::AllowOrigin;

    // Allow specific origins based on environment
    // P1-SECURITY: Never use AllowOrigin::any() - even in development
    let allowed_origins = if std::env::var("PRODUCTION").is_ok() {
        AllowOrigin::list([
            "https://freshcredit.com".parse().unwrap(),
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            "https://app.freshcredit.com".parse().unwrap(),
            "https://testnet.freshcredit.com".parse().unwrap(),
        ])
    } else {
        // Development: restrict to known local origins
        AllowOrigin::list([
            "http://localhost:3000".parse().unwrap(),
            "http://localhost:3001".parse().unwrap(),
            "http://localhost:5173".parse().unwrap(),
            "http://127.0.0.1:3000".parse().unwrap(),
            "http://127.0.0.1:3001".parse().unwrap(),
            "http://127.0.0.1:5173".parse().unwrap(),
        ])
    };

    CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([
            http::Method::GET,
            http::Method::POST,
            http::Method::PUT,
            http::Method::DELETE,
        ])
        .allow_headers([
            http::header::CONTENT_TYPE,
            http::header::AUTHORIZATION,
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            http::header::ACCEPT,
        ])
}

/// Unified storage backend that can use either legacy JSON or native NOMT
#[derive(Debug)]
pub enum StorageBackend {
    /// Legacy JSON storage (for NFS/Filestore)
    Legacy(NomtStorage),
    /// Native NOMT storage (for local `NVMe`)
    Native(NativeNomtStorage),
}

impl StorageBackend {
    /// Store data and return response
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn store(
        &mut self,
        user_id: &str,
        report_type: &str,
        data: &[u8],
    ) -> anyhow::Result<StoreResponse> {
        match self {
            Self::Legacy(s) => s.store(user_id, report_type, data).await,
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            Self::Native(s) => s.store(user_id, report_type, data).await,
        }
    }

    /// Verify a hash (legacy doesn't support `expected_root` parameter)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn verify(
        &mut self,
        hash: &str,
        expected_root: Option<&str>,
    ) -> anyhow::Result<VerifyResponse> {
        match self {
            Self::Legacy(s) => s.verify(hash).await,
            Self::Native(s) => s.verify(hash, expected_root).await,
        }
    }

    /// Get proof for a hash
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_proof(&mut self, hash: &str) -> anyhow::Result<Option<NomtProof>> {
        match self {
            Self::Legacy(s) => s.get_proof(hash).await,
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            Self::Native(_) => {
                // Native NOMT generates proofs during store, not on-demand
                // For now, return None - proofs are returned in StoreResponse
                Ok(None)
            }
        }
    }

    /// Get storage statistics as JSON
    pub async fn get_stats(&mut self) -> serde_json::Value {
        match self {
            Self::Legacy(s) => s.get_stats().await,
            Self::Native(s) => {
                // Convert native stats to JSON
                // FIXED: Made async to avoid block_on anti-pattern
                let stats = s.stats().await;
                serde_json::json!({
                    "total_hashes": stats.total_hashes,
                    "total_data_bytes": stats.total_data_bytes,
                    "current_root": stats.current_root,
                    "unique_users": stats.unique_users,
                    "efficiency_ratio": stats.efficiency_ratio,
                    "storage_mode": "native_nomt"
                })
            }
        }
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001

    /// Check if storage is ready
    pub const fn is_ready(&self) -> bool {
        match self {
            Self::Legacy(s) => s.is_ready(),
            Self::Native(s) => s.is_ready(),
        }
    }

    /// Get current root hash
    pub fn current_root(&self) -> String {
        match self {
            Self::Legacy(s) => s.current_root(),
            Self::Native(s) => s.current_root(),
        }
    }

    /// Get Prometheus metrics
    pub async fn get_metrics(&mut self) -> String {
        match self {
            Self::Legacy(s) => s.get_metrics().await,
            Self::Native(s) => {
                // Generate Prometheus-style metrics for native storage
                // FIXED: Made async to avoid block_on anti-pattern
                let stats = s.stats().await;
                format!(
                    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
                    "# HELP nomt_total_hashes Total number of hashes stored\n\
                     # TYPE nomt_total_hashes gauge\n\
                     nomt_total_hashes {}\n\
                     # HELP nomt_total_bytes Total bytes stored\n\
                     # TYPE nomt_total_bytes gauge\n\
                     nomt_total_bytes {}\n\
                     # HELP nomt_unique_users Number of unique users\n\
                     # TYPE nomt_unique_users gauge\n\
                     nomt_unique_users {}\n\
                     # HELP nomt_storage_mode Storage backend mode (1=native)\n\
                     # TYPE nomt_storage_mode gauge\n\
                     nomt_storage_mode 1\n",
                    stats.total_hashes, stats.total_data_bytes, stats.unique_users
                )
            }
        }
    }
}

/// Application state shared across handlers
struct AppState {
    storage: RwLock<StorageBackend>,
    anchoring: AnchoringService,
    peer_sync: PeerSyncService,
}

/// Initialize tracing with JSON formatting and an INFO-level default.
fn init_tracing() {
    tracing_subscriber::registry()
        .with(fmt::layer().json())
        .with(EnvFilter::from_default_env().add_directive(Level::INFO.into()))
        .init();
}

/// Load `.env` overrides when not running in production.
fn load_dotenv() {
    if std::env::var("ENVIRONMENT").unwrap_or_default() != "production" {
        dotenvy::dotenv().ok();
    }
}

/// Read base configuration from environment variables.
fn read_base_config() -> (String, String, String, bool) {
    let data_dir = std::env::var("NOMT_DATA_DIR").unwrap_or_else(|_| "/data/nomt".to_string());
    let http_port = std::env::var("NOMT_HTTP_PORT").unwrap_or_else(|_| "8081".to_string());
    let grpc_port = std::env::var("NOMT_GRPC_PORT").unwrap_or_else(|_| "50051".to_string());
    let use_native_nomt = std::env::var("NOMT_NATIVE")
        .map(|s| s == "true" || s == "1")
        .unwrap_or(false);
    (data_dir, http_port, grpc_port, use_native_nomt)
}

/// Initialize NOMT storage in native or legacy mode.
///
/// Native NOMT requires local disk (`io_uring` doesn't work with NFS). Falls
/// back to legacy storage if native initialization fails.
async fn init_storage(data_dir: &str, use_native_nomt: bool) -> anyhow::Result<StorageBackend> {
    if use_native_nomt {
        info!("Attempting native NOMT storage (Beatree + Bitbox)");
        match NativeNomtStorage::new(data_dir).await {
            Ok(native) => {
                info!("Native NOMT storage initialized successfully");
                Ok(StorageBackend::Native(native))
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Native NOMT failed (likely NFS - io_uring incompatible), falling back to legacy storage"
                );
                Ok(StorageBackend::Legacy(NomtStorage::new(data_dir).await?))
            }
        }
    } else {
        info!("Initializing legacy JSON storage (NFS-compatible)");
        Ok(StorageBackend::Legacy(NomtStorage::new(data_dir).await?))
    }
}

/// Build the anchoring configuration from environment variables.
fn build_anchoring_config() -> AnchoringConfig {
    // HARDCODED_URL: Default blockchain API URL - override with BLOCKCHAIN_API_URL env var
    let blockchain_api_url = std::env::var("BLOCKCHAIN_API_URL")
        .unwrap_or_else(|_| "https://blockchain.freshcredit.com".to_string());
    // HARDCODED_LIMIT: Default anchor after N stores - override with ANCHOR_INTERVAL_STORES env var
    let anchor_interval_stores = std::env::var("ANCHOR_INTERVAL_STORES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    // HARDCODED_TIMEOUT: Default anchor interval in seconds - override with ANCHOR_INTERVAL_SECONDS env var
    let anchor_interval_seconds = std::env::var("ANCHOR_INTERVAL_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let anchoring_enabled = std::env::var("ANCHORING_ENABLED")
        .map(|s| s == "true" || s == "1")
        .unwrap_or(true);

    AnchoringConfig {
        // HARDCODED_URL: Local Substrate RPC endpoint - production uses Kubernetes service DNS
        substrate_rpc_url: "http://localhost:9944".to_string(),
        blockchain_api_url,
        anchor_interval_stores,
        anchor_interval_seconds,
        enabled: anchoring_enabled,
    }
}

/// Build the peer sync configuration from environment variables.
///
/// Returns the configuration, whether peer sync is enabled, and the local pod
/// name for logging.
fn build_peer_sync_config(http_port: &str) -> (PeerSyncConfig, bool, String) {
    let peer_sync_enabled = std::env::var("PEER_SYNC_ENABLED")
        .map(|s| s == "true" || s == "1")
        .unwrap_or(false);
    let headless_service = std::env::var("PEER_SYNC_HEADLESS_SERVICE")
        .unwrap_or_else(|_| "freshcredit-blockchain-headless".to_string());
    let namespace = std::env::var("PEER_SYNC_NAMESPACE")
        .unwrap_or_else(|_| "freshcredit-blockchain".to_string());
    let pod_name = std::env::var("POD_NAME").unwrap_or_default();

    let config = PeerSyncConfig {
        headless_service,
        namespace,
        port: http_port.parse().unwrap_or(8081),
        pod_name: pod_name.clone(),
        enabled: peer_sync_enabled,
    };
    (config, peer_sync_enabled, pod_name)
}

// TAG: surface=blockchain owner=blockchain-team rule=BC-001
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    info!("Starting NOMT Sidecar Service v0.1.0");

    load_dotenv();
    let (data_dir, http_port, grpc_port, use_native_nomt) = read_base_config();
    info!(data_dir = %data_dir, http_port = %http_port, grpc_port = %grpc_port, "Configuration loaded");

    let storage = init_storage(&data_dir, use_native_nomt).await?;

    let anchoring_config = build_anchoring_config();
    let anchoring = AnchoringService::new(anchoring_config.clone());
    info!(
        enabled = anchoring_config.enabled,
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        interval_stores = anchoring_config.anchor_interval_stores,
        interval_seconds = anchoring_config.anchor_interval_seconds,
        "Anchoring service initialized"
    );

    let (peer_sync_config, peer_sync_enabled, pod_name) = build_peer_sync_config(&http_port);
    let peer_sync = PeerSyncService::new(peer_sync_config);
    info!(
        enabled = peer_sync_enabled,
        pod_name = %pod_name,
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        "Peer sync service initialized"
    );

    let state = Arc::new(AppState {
        storage: RwLock::new(storage),
        anchoring,
        peer_sync,
    });

    // Discover peers on startup if enabled
    if peer_sync_enabled {
        let state_clone = state.clone();
        tokio::spawn(async move {
            // Wait a bit for other pods to be ready
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            state_clone.peer_sync.discover_peers().await;
        });
    }

    // Build HTTP router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .route("/metrics", get(metrics))
        .route("/store", post(store_report))
        .route("/sync/store", post(sync_store)) // Peer sync endpoint
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        .route("/verify/:hash", get(verify_hash))
        .route("/proof/:hash", get(get_proof))
        .route("/stats", get(get_stats))
        .route("/anchoring/stats", get(get_anchoring_stats))
        .route("/peer/discover", post(discover_peers)) // Manual peer discovery
        .layer(cors_layer())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start HTTP server
    let addr = format!("0.0.0.0:{http_port}");
    info!(addr = %addr, "Starting HTTP server");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cors_layer_does_not_panic() {
        let _layer = cors_layer();
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
