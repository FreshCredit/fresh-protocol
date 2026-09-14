use super::*;

// ============================================================================
// HTML Handlers
// ============================================================================

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
pub(super) async fn explorer_index(State(state): State<AppState>) -> impl IntoResponse {
    let client = state.client.read().await;

    // Get stats if client is connected, otherwise use defaults
    let connected = client.is_some();
    let stats_json = if let Some(ref client) = *client {
        client
            .get_stats()
            .await
            .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    drop(client); // Release lock before fetching blocks

    let recent_blocks = fetch_recent_blocks(&state).await;

    let template = IndexTemplate {
        block_height: stats_json
            .get("block_height")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        total_hashes: stats_json
            .get("total_anchored")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        is_syncing: stats_json
            .get("is_syncing")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        connected,
        peers: stats_json
            .get("peers")
            .and_then(serde_json::Value::as_u64)
            .map_or(0, |v| u32::try_from(v).unwrap_or(0)),
        recent_blocks,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
pub(super) async fn explorer_verify() -> impl IntoResponse {
    Html(
        VerifyTemplate {}
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

pub(super) async fn explorer_api_docs() -> impl IntoResponse {
    Html(
        ApiDocsTemplate {}
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
pub(super) async fn explorer_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let client = state.client.read().await;

    // Check if client is connected
    let connected = client.is_some();
    let (healthy, stats_json) = if let Some(ref client) = *client {
        let healthy = client.health_check().await.unwrap_or(false);
        let stats_json = client
            .get_stats()
            .await
            .unwrap_or_else(|_| serde_json::json!({}));
        (healthy, stats_json)
    } else {
        (false, serde_json::json!({}))
    };

    let template = MetricsTemplate {
        is_healthy: healthy,
        block_height: stats_json
            .get("block_height")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        total_hashes: stats_json
            .get("total_anchored")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        peers: stats_json
            .get("peers")
            .and_then(serde_json::Value::as_u64)
            .map_or(0, |v| u32::try_from(v).unwrap_or(0)),
        is_syncing: stats_json
            .get("is_syncing")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        connected,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

pub(super) async fn explorer_block_detail(
    Path(number): Path<u32>,
    State(state): State<AppState>,
    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
) -> impl IntoResponse {
    let client = state.client.read().await;

    // Check if client is initialized
    let Some(ref client) = *client else {
        return Html(
            "<h1>Blockchain Explorer</h1><p>Blockchain connection not yet established. Please retry shortly.</p>"
                .to_string()
        );
    };

    match client.get_block(number).await {
        Ok(block) => {
            let extrinsics: Vec<ExtrinsicInfo> = block
                .get("extrinsics")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|e| {
                            Some(ExtrinsicInfo {
                                index: usize::try_from(e.get("index")?.as_u64()?).ok()?,
                                hex: e.get("hex")?.as_str()?.to_string(),
                                length: usize::try_from(e.get("length")?.as_u64()?).ok()?,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            let block_number = block
                .get("number")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let template = BlockDetailTemplate {
                block_number,
                block_hash: block
                    .get("hash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                parent_hash: block
                    .get("parent_hash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                parent_block_number: block_number.saturating_sub(1),
                next_block_number: block_number + 1,
                // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
                state_root: block
                    .get("state_root")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                extrinsics_root: block
                    .get("extrinsics_root")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                extrinsics_count: extrinsics.len(),
                extrinsics,
            };
            Html(
                template
                    .render()
                    .unwrap_or_else(|e| format!("Template error: {e}")),
            )
        }
        Err(e) => {
            // MED-001 FIXED: Sanitized error message
            tracing::error!("Block not found: {}", e);
            Html("<h1>Block Not Found</h1><p>The requested block could not be retrieved. Please try again later.</p>".to_string())
        }
    }
}

pub(super) async fn explorer_hash_detail(
    Path(hash): Path<String>,
    State(state): State<AppState>,
    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
) -> impl IntoResponse {
    let client = state.client.read().await;

    // Check if client is initialized
    let Some(ref client) = *client else {
        return Html(
            "<h1>Blockchain Explorer</h1><p>Blockchain connection not yet established. Please retry shortly.</p>"
                .to_string()
        );
    };

    if let Ok(result) = client.verify_hash(&hash).await {
        let template = HashDetailTemplate {
            hash: result.hash,
            verified: result.verified,
            block_number: result.block_number,
            user_id: result.user_id,
            report_type: result.report_type,
            timestamp: result.timestamp,
        };
        Html(
            template
                .render()
                .unwrap_or_else(|e| format!("Template error: {e}")),
        )
    } else {
        let template = HashDetailTemplate {
            hash,
            verified: false,
            block_number: None,
            user_id: None,
            report_type: None,
            timestamp: None,
        };
        Html(
            template
                .render()
                .unwrap_or_else(|e| format!("Template error: {e}")),
        )
    }
}
