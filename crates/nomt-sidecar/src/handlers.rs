// TAG: surface=blockchain owner=blockchain-team rule=BC-001
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::types::{NomtProof, StoreRequest, StoreResponse, VerifyResponse};
use crate::AppState;

/// Health check endpoint
pub async fn health_check() -> &'static str {
    "OK"
}

/// Readiness check - verifies NOMT storage is accessible
pub async fn readiness_check(
    State(state): State<Arc<AppState>>,
) -> Result<&'static str, StatusCode> {
    let storage = state.storage.read().await;
    if storage.is_ready() {
        Ok("READY")
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

/// Prometheus metrics endpoint
/// Combines storage and anchoring metrics in Prometheus format
/// Uses write lock because `get_metrics` reloads from shared NFS storage
pub async fn metrics(State(state): State<Arc<AppState>>) -> String {
    let mut storage = state.storage.write().await;
    let storage_metrics = storage.get_metrics().await;
    let anchoring_metrics = state.anchoring.get_prometheus_metrics();

    format!("{storage_metrics}{anchoring_metrics}")
}

/// Store report data and generate Merkle proof
pub async fn store_report(
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    State(state): State<Arc<AppState>>,
    Json(request): Json<StoreRequest>,
) -> Result<Json<StoreResponse>, StatusCode> {
    let mut storage = state.storage.write().await;

    match storage
        .store(&request.user_id, &request.report_type, &request.data)
        .await
    {
        Ok(result) => {
            // Broadcast to peers (fire and forget)
            if state.peer_sync.is_enabled() {
                state.peer_sync.broadcast_store(&request).await;
            }

            // Check if we should anchor the root to Substrate
            if state.anchoring.notify_store().await {
                let root = result.nomt_root.clone();
                let user_id = request.user_id.clone();
                let anchoring = &state.anchoring;

                // Spawn anchoring in background (don't block the response)
                let anchor_result = anchoring.anchor_root(&root, &user_id).await;
                if let Ok(ar) = anchor_result {
                    if ar.success {
                        info!(root = %root, "NOMT root anchored to Substrate");
                    }
                }
            }
            Ok(Json(result))
        }
        Err(e) => {
            warn!(error = %e, "Failed to store report");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Sync store endpoint - receives broadcasts from peers
/// Does NOT re-broadcast to avoid infinite loops
pub async fn sync_store(
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    State(state): State<Arc<AppState>>,
    Json(request): Json<StoreRequest>,
) -> Result<Json<StoreResponse>, StatusCode> {
    debug!("Received sync store from peer");
    let mut storage = state.storage.write().await;

    match storage
        .store(&request.user_id, &request.report_type, &request.data)
        .await
    {
        Ok(result) => {
            // Do NOT broadcast to peers - this is already a sync from a peer
            // Do NOT anchor - only the originating pod anchors
            Ok(Json(result))
        }
        Err(e) => {
            warn!(error = %e, "Failed to sync store from peer");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Manual peer discovery endpoint
pub async fn discover_peers(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let peers = state.peer_sync.discover_peers().await;
    Json(serde_json::json!({
        "peers": peers,
        "count": peers.len()
    }))
}

/// Verify a hash exists in NOMT storage
/// Uses write lock because verify reloads from shared NFS storage
pub async fn verify_hash(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> Result<Json<VerifyResponse>, StatusCode> {
    let mut storage = state.storage.write().await;

    match storage.verify(&hash, None).await {
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            warn!(error = %e, hash = %hash, "Verification failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get Merkle proof for a hash
/// Uses write lock because `get_proof` reloads from shared NFS storage
pub async fn get_proof(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> Result<Json<NomtProof>, StatusCode> {
    let mut storage = state.storage.write().await;

    match storage.get_proof(&hash).await {
        Ok(Some(proof)) => Ok(Json(proof)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            warn!(error = %e, hash = %hash, "Failed to get proof");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get storage statistics
/// Uses write lock because `get_stats` reloads from shared NFS storage
pub async fn get_stats(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut storage = state.storage.write().await;
    Json(storage.get_stats().await)
}

/// Get anchoring statistics
pub async fn get_anchoring_stats(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let anchoring_stats = state.anchoring.get_stats().await;
    let last_root = state.anchoring.get_last_anchored_root().await;
    Json(serde_json::json!({
        "anchoring": anchoring_stats,
        "last_anchored_root": last_root
    }))
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
