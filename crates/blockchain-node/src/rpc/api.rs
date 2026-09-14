use super::*;

// ============================================================================
// JSON API Handlers
// ============================================================================

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
pub(super) async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.client.read().await;

    // Check if client is initialized
    let Some(ref client) = *client_guard else {
        let status = state.connection_status.read().await;
        return Json(serde_json::json!({
            "status": "initializing",
            "message": "Blockchain connection not yet established",
            "connection_status": format!("{:?}", *status),
            "node": "substrate-freshcredit"
        }));
    };

    match client.health_check().await {
        Ok(healthy) => Json(serde_json::json!({
            "status": if healthy { "healthy" } else { "syncing" },
            "node": "substrate-freshcredit"
        })),
        Err(e) => {
            // Connection error - try to reconnect
            // Drop the read lock before trying to reconnect
            drop(client_guard);
            if try_reconnect(&state).await.is_ok() {
                let client_guard = state.client.read().await;
                if let Some(ref client) = *client_guard {
                    if let Ok(healthy) = client.health_check().await {
                        return Json(serde_json::json!({
                            "status": if healthy { "healthy" } else { "syncing" },
                            "node": "substrate-freshcredit"
                        }));
                    }
                }
            }
            Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            }))
        }
    }
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
pub(super) async fn get_stats(State(state): State<AppState>) -> impl IntoResponse {
    // First try with existing client
    let client_guard = state.client.read().await;

    // Check if client is initialized
    let Some(ref client) = *client_guard else {
        return Json(serde_json::json!({
            "error": "Blockchain connection not yet established - please retry shortly"
        }));
    };

    match client.get_stats().await {
        Ok(statistics) => Json(statistics),
        Err(e) => {
            let error_str = e.to_string();
            info!("get_stats failed with error: {}", error_str);

            // Check if this is a connection error
            if error_str.contains("connection closed")
                || error_str.contains("background task")
                || error_str.contains("restart required")
                || error_str.contains("closed")
            {
                info!("Detected connection error, attempting reconnect...");
                drop(client_guard);
                if try_reconnect(&state).await.is_ok() {
                    info!("Reconnect successful, retrying get_stats...");
                    let client_guard = state.client.read().await;
                    if let Some(ref client) = *client_guard {
                        if let Ok(statistics) = client.get_stats().await {
                            return Json(statistics);
                        }
                    }
                }
            }
            Json(serde_json::json!({
                "error": error_str
            }))
        }
    }
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
pub(super) async fn get_recent_blocks(State(state): State<AppState>) -> impl IntoResponse {
    let client = state.client.read().await;

    // Check if client is initialized
    let Some(ref client) = *client else {
        return Json(serde_json::json!({
            "error": "Blockchain connection not yet established - please retry shortly"
        }));
    };

    match client.get_recent_blocks(API_RECENT_BLOCKS_COUNT).await {
        Ok(blocks) => Json(serde_json::json!({ "blocks": blocks })),
        Err(e) => {
            // MED-001 FIXED: Sanitized error message - don't leak internal details
            tracing::error!("Failed to get recent blocks: {}", e);
            Json(serde_json::json!({
                "error": "Failed to retrieve blocks. Please try again later."
            }))
        }
    }
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
pub(super) async fn get_blocks_partial(State(state): State<AppState>) -> impl IntoResponse {
    let recent_blocks = fetch_recent_blocks(&state).await;
    let template = BlocksPartialTemplate { recent_blocks };
    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

/// Unified hash verification API
///
/// Implements cascading verification for ALL hash types:
/// 1. Block hash (check if it's a valid Substrate block hash)
/// 2. NOMT sidecar (fast, Merkle proof available)
/// 3. Report hash via `FreshCredit` pallet (anchored data hashes)
pub(super) async fn verify_hash_api(
    Path(hash): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Normalize hash - remove 0x prefix if present for consistent handling
    let hash_normalized = hash.trim_start_matches("0x").to_lowercase();

    // Step 1: Check if it's a block hash
    {
        let client = state.client.read().await;

        // Check if client is initialized
        let Some(ref client) = *client else {
            return Json(serde_json::json!({
                "verified": false,
                "error": "Blockchain connection not yet established - please retry shortly"
            }));
        };

        match client.get_block_by_hash(&hash_normalized).await {
            Ok(Some(block)) => {
                info!("Hash verified as block hash: {}", hash);
                return Json(serde_json::json!({
                                    "verified": true,
                                    "hash": format!("0x{}", hash_normalized),
                // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
                                    "source": "block",
                                    "hash_type": "block_hash",
                                    "block_number": block.get("number"),
                                    "parent_hash": block.get("parent_hash"),
                                    "state_root": block.get("state_root"),
                                    "extrinsics_root": block.get("extrinsics_root"),
                                    "extrinsics_count": block.get("extrinsics_count")
                                }));
            }
            Ok(None) => {
                // Not a block hash, continue to other checks
            }
            Err(e) => {
                warn!("Error checking block hash: {}", e);
            }
        }
    }

    // Step 2: Try NOMT sidecar (if configured)
    if let Some(ref nomt_url) = state.nomt_sidecar_url {
        match verify_hash_via_nomt(&state.http_client, nomt_url, &hash_normalized).await {
            Ok(Some(nomt_result)) => {
                info!("Hash verified via NOMT sidecar: {}", hash);
                return Json(serde_json::json!({
                    "verified": true,
                    "hash": hash_normalized,
                    "source": "nomt",
                    "hash_type": "report_hash",
                    "root": nomt_result.root,
                    "proof_available": nomt_result.proof_available,
                    "timestamp": nomt_result.timestamp
                }));
            }
            Ok(None) => {
                // Hash not found in NOMT, fall through to report hash check
                info!("Hash not found in NOMT, checking report hashes: {}", hash);
            }
            Err(e) => {
                warn!(
                    "NOMT verification failed, falling back to report hash check: {}",
                    e
                );
            }
        }
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    // Step 3: Check if it's an anchored report hash
    let client = state.client.read().await;

    // Check if client is initialized
    let Some(ref client) = *client else {
        return Json(serde_json::json!({
            "verified": false,
            "error": "Blockchain connection not yet established - please retry shortly"
        }));
    };

    match client.verify_hash(&hash_normalized).await {
        Ok(result) if result.verified => Json(serde_json::json!({
            "verified": true,
            "hash": result.hash,
            "source": "substrate",
            "hash_type": "report_hash",
            "block_number": result.block_number,
            "timestamp": result.timestamp,
            "user_id": result.user_id,
            "report_type": result.report_type
        })),
        Ok(result) => {
            // Hash not found in any source
            Json(serde_json::json!({
                "verified": false,
                "hash": result.hash,
                "message": "Hash not found. This hash is not a valid block hash, NOMT hash, or anchored report hash on this chain."
            }))
        }
        Err(e) => {
            // MED-001 FIXED: Sanitized error message
            tracing::error!("Hash verification failed: {}", e);
            Json(serde_json::json!({
                "verified": false,
                "hash": hash_normalized,
                "error": "Verification failed. Please try again later."
            }))
        }
    }
}

#[allow(dead_code)]
pub(super) async fn get_user_hashes(
    Path(user_id): Path<String>,
    State(state): State<AppState>,
    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
) -> impl IntoResponse {
    let client = state.client.read().await;

    // Check if client is initialized
    let Some(ref client) = *client else {
        return Json(serde_json::json!({
            "user_id": user_id,
            "count": 0,
            "hashes": [],
            "error": "Blockchain connection not yet established - please retry shortly"
        }));
    };

    match client.get_user_hashes(&user_id).await {
        Ok(hashes) => {
            let list: Vec<serde_json::Value> = hashes
                .iter()
                .map(|h| {
                    serde_json::json!({
                        "id": h.id,
                        "user_id": h.user_id,
                        "report_type": h.report_type,
                        "hash": h.hash,
                        "block_number": h.block_number,
                        "timestamp": h.timestamp
                    })
                })
                // TAG: surface=blockchain owner=platform-team rule=GENERAL-001
                .collect();
            Json(serde_json::json!({
                "user_id": user_id,
                "count": list.len(),
                "hashes": list
            }))
        }
        Err(e) => {
            // MED-001 FIXED: Sanitized error message
            tracing::error!("Failed to get user hashes: {}", e);
            Json(serde_json::json!({
                "user_id": user_id,
                "count": 0,
                "hashes": [],
                "error": "Failed to retrieve hashes. Please try again later."
            }))
        }
    }
}

/// Secured version of `get_user_hashes` with authorization check
///
/// Users can only access their own hashes. Service accounts can access any user's hashes.
pub(super) async fn get_user_hashes_secured(
    Path(requested_user_id): Path<String>,
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<crate::config::Claims>,
    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
) -> impl IntoResponse {
    // Authorization check: users can only access their own data
    // Service accounts (token_type = "service") can access any user
    if claims.token_type != "service" && claims.sub != requested_user_id {
        warn!(
            requester = %claims.sub,
            requested_user = %requested_user_id,
            "Unauthorized access attempt to user hashes"
        );
        return Json(serde_json::json!({
            "error": "Forbidden",
            "message": "You can only access your own hashes"
        }));
    }

    // Proceed with the request
    let client = state.client.read().await;

    let Some(ref client) = *client else {
        return Json(serde_json::json!({
            "user_id": requested_user_id,
            "count": 0,
            "hashes": [],
            "error": "Blockchain connection not yet established - please retry shortly"
        }));
    };

    info!(
        user_id = %requested_user_id,
        requester = %claims.sub,
        token_type = %claims.token_type,
        "Fetching user hashes (authorized)"
    );

    match client.get_user_hashes(&requested_user_id).await {
        Ok(hashes) => {
            let list: Vec<serde_json::Value> = hashes
                .iter()
                .map(|h| {
                    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
                    serde_json::json!({
                        "id": h.id,
                        "user_id": h.user_id,
                        "report_type": h.report_type,
                        "hash": h.hash,
                        "block_number": h.block_number,
                        "timestamp": h.timestamp
                    })
                })
                .collect();
            Json(serde_json::json!({
                "user_id": requested_user_id,
                "count": list.len(),
                "hashes": list
            }))
        }
        Err(e) => {
            error!(
                user_id = %requested_user_id,
                error = %e,
                "Failed to fetch user hashes"
            );
            Json(serde_json::json!({
                "user_id": requested_user_id,
                "count": 0,
                "hashes": [],
                "error": e.to_string()
            }))
        }
    }
}

pub(super) async fn anchor_hash(
    State(state): State<AppState>,
    Json(req): Json<crate::config::AnchorRequest>,
    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
) -> impl IntoResponse {
    // Validate request
    if let Err(e) = req.validate() {
        return Json(serde_json::json!({
            "success": false,
            "error": e
        }));
    }

    let client_guard = state.client.read().await;

    // Check if client is initialized
    let Some(ref client) = *client_guard else {
        return Json(serde_json::json!({
            "success": false,
            "error": "Blockchain connection not yet established - please retry shortly"
        }));
    };

    let create_request = CreateHashRequest {
        user_id: req.user_id.clone(),
        report_type: req.report_type.clone(),
        report_data: serde_json::json!(req.report_data),
    };
    // Bytes exactly as create_hash anchors them (SHA256 of the JSON-quoted
    // report_data string) — the sidecar keys entries on SHA256(data), so this
    // makes the on-chain hash resolvable via /proof/:hash.
    let nomt_store_bytes = serde_json::to_string(&req.report_data).map(String::into_bytes);
    let nomt_user = req.user_id.clone();
    let nomt_type = req.report_type.clone();
    let nomt_url = state.nomt_sidecar_url.clone();
    let nomt_client = state.http_client.clone();
    let store_via_nomt = move || {
        if let (Some(url), Ok(bytes)) = (&nomt_url, &nomt_store_bytes) {
            crate::helpers::spawn_nomt_store(
                &nomt_client,
                url,
                nomt_user.clone(),
                nomt_type.clone(),
                bytes.clone(),
            );
        }
    };
    match client.create_hash(create_request).await {
        Ok(result) => {
            store_via_nomt();
            Json(serde_json::json!({
                "success": true,
                "hash": result.hash,
                "block_number": result.block_number
            }))
        }
        Err(e) => {
            let error_str = e.to_string();
            // A dead WS background task ("restart required") kills every
            // anchor until something reconnects — previously only /health and
            // /stats did that, so anchors failed until pod restart. Reconnect
            // on connection-class errors and retry once, same as get_stats.
            if error_str.contains("connection closed")
                || error_str.contains("background task")
                || error_str.contains("restart required")
                || error_str.contains("closed")
            {
                info!("anchor_hash detected connection error, attempting reconnect...");
                drop(client_guard);
                if try_reconnect(&state).await.is_ok() {
                    info!("Reconnect successful, retrying anchor_hash...");
                    let retry_guard = state.client.read().await;
                    if let Some(ref retry_client) = *retry_guard {
                        let retry_request = CreateHashRequest {
                            user_id: req.user_id,
                            report_type: req.report_type,
                            report_data: serde_json::json!(req.report_data),
                        };
                        if let Ok(result) = retry_client.create_hash(retry_request).await {
                            store_via_nomt();
                            return Json(serde_json::json!({
                                "success": true,
                                "hash": result.hash,
                                "block_number": result.block_number
                            }));
                        }
                    }
                }
            }
            Json(serde_json::json!({
                "success": false,
                "error": error_str
            }))
        }
    }
}
