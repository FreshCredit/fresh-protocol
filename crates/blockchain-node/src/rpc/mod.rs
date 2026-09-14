//! RPC server handlers and route configuration.

use crate::config::{API_RECENT_BLOCKS_COUNT, BLOCKS_PARTIAL_COUNT, MAX_REQUEST_BODY_SIZE};
use crate::helpers::{
    api_key_auth_middleware,
    build_cors_layer,
    build_rate_limit_layer,
    combined_auth_middleware,
    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    verify_hash_via_nomt,
};
use crate::node::{try_reconnect, AppState};
use askama::Template;
use axum::{
    extract::{Path, State},
    http::{header, HeaderValue},
    middleware,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use blockchain_client::CreateHashRequest;
use serde::Serialize;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::{error, info, warn, Level};

mod api;
mod html;

use api::*;
use html::*;

// ============================================================================
// Templates
// ============================================================================

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[derive(Template)]
#[template(path = "explorer/index.html")]
pub struct IndexTemplate {
    pub block_height: u64,
    pub total_hashes: u64,
    pub is_syncing: bool,
    pub connected: bool,
    pub peers: u32,
    pub recent_blocks: Vec<BlockInfo>,
}

#[derive(Template)]
#[template(path = "explorer/blocks_partial.html")]
pub struct BlocksPartialTemplate {
    pub recent_blocks: Vec<BlockInfo>,
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Block information for display
#[derive(Clone, Serialize)]
pub struct BlockInfo {
    pub number: u64,
    pub hash: String,
    pub hash_short: String,
    pub parent_hash_short: String,
    pub extrinsics_count: usize,
}

#[derive(Template)]
#[template(path = "explorer/verify.html")]
pub struct VerifyTemplate {}

#[derive(Template)]
#[template(path = "explorer/api.html")]
pub struct ApiDocsTemplate {}

#[derive(Template)]
#[template(path = "explorer/metrics.html")]
pub struct MetricsTemplate {
    pub is_healthy: bool,
    pub block_height: u64,
    pub total_hashes: u64,
    pub peers: u32,
    pub is_syncing: bool,
    pub connected: bool,
}

#[derive(Template)]
#[template(path = "explorer/block.html")]
pub struct BlockDetailTemplate {
    pub block_number: u64,
    pub block_hash: String,
    pub parent_hash: String,
    pub parent_block_number: u64,
    pub next_block_number: u64,
    pub state_root: String,
    pub extrinsics_root: String,
    pub extrinsics_count: usize,
    pub extrinsics: Vec<ExtrinsicInfo>,
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[derive(Clone, Serialize)]
pub struct ExtrinsicInfo {
    pub index: usize,
    pub hex: String,
    pub length: usize,
}

#[derive(Template)]
#[template(path = "explorer/hash.html")]
pub struct HashDetailTemplate {
    pub hash: String,
    pub verified: bool,
    pub block_number: Option<u64>,
    pub user_id: Option<String>,
    pub report_type: Option<String>,
    pub timestamp: Option<String>,
}

// ============================================================================
// Helpers
// ============================================================================

/// Helper to fetch recent blocks and convert to `BlockInfo`
pub async fn fetch_recent_blocks(state: &AppState) -> Vec<BlockInfo> {
    let client = state.client.read().await;

    // Check if client is initialized
    let Some(ref client) = *client else {
        return Vec::new();
    };
    // TAG: surface=blockchain owner=platform-team rule=GENERAL-001

    client
        .get_recent_blocks(u32::try_from(BLOCKS_PARTIAL_COUNT).unwrap_or(0))
        .await
        .map_or_else(
            |_| Vec::new(),
            |blocks| {
                blocks
                    .iter()
                    .filter_map(|b| {
                        let number = b.get("number")?.as_u64()?;
                        let hash = b.get("hash")?.as_str()?.to_string();
                        let parent_hash = b.get("parent_hash")?.as_str()?.to_string();
                        let extrinsics_count =
                            usize::try_from(b.get("extrinsics_count")?.as_u64()?).ok()?;

                        Some(BlockInfo {
                            number,
                            hash: hash.clone(),
                            hash_short: format!("{}...{}", &hash[..10], &hash[hash.len() - 8..]),
                            parent_hash_short: format!(
                                "{}...{}",
                                &parent_hash[..10],
                                &parent_hash[parent_hash.len() - 8..]
                            ),
                            extrinsics_count,
                        })
                    })
                    .collect()
            },
        )
}

// ============================================================================
// Route Configuration
// ============================================================================

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
pub fn create_app(state: AppState) -> Router {
    // Public routes - no authentication required (read-only, non-sensitive)
    let public_routes = Router::new()
        .route("/", get(explorer_index))
        .route("/verify", get(explorer_verify))
        .route("/api", get(explorer_api_docs))
        .route("/health", get(health_check))
        .route("/api/hash/:hash", get(verify_hash_api))
        .route("/block/:number", get(explorer_block_detail))
        .route("/hash/:hash", get(explorer_hash_detail))
        .route("/blocks/partial", get(get_blocks_partial))
        // Serve static files (CSS, JS) for the explorer UI
        .nest_service("/static", ServeDir::new("static"));

    // Protected routes - authentication required (write operations, sensitive data)
    let protected_routes = Router::new()
        .route("/anchor", post(anchor_hash))
        .route("/user/:user_id/hashes", get(get_user_hashes_secured))
        // Apply authentication middleware
        .layer(middleware::from_fn_with_state(
            state.clone(),
            combined_auth_middleware,
        ));

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    // Metrics and stats - API key only (for monitoring systems)
    let metrics_routes = Router::new()
        .route("/metrics", get(explorer_metrics))
        .route("/stats", get(get_stats))
        .route("/blocks/recent", get(get_recent_blocks))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_auth_middleware,
        ));

    // Combine all routes with global middleware
    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(metrics_routes)
        // Global middleware (applied to all routes)
        // Note: Layers are applied right-to-left, so order matters:
        // Request: trace -> body_limit -> security_headers -> cors -> rate_limit -> handler
        // Response: handler -> rate_limit -> cors -> security_headers -> body_limit -> trace
        .layer(build_rate_limit_layer())
        .layer(build_cors_layer())
        // Security headers (applied individually to avoid type complexity)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
            header::HeaderName::from_static("x-xss-protection"),
            HeaderValue::from_static("1; mode=block"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        // P0 SECURITY FIX: Strict CSP without unsafe-inline/unsafe-eval
        // Inline scripts moved to external files: theme-init.js, theme.js, modal.js
        .layer(SetResponseHeaderLayer::if_not_present(
            header::HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                "default-src 'self'; \
                 script-src 'self'; \
                 style-src 'self' https://fonts.googleapis.com; \
                 font-src 'self' https://fonts.gstatic.com; \
                 img-src 'self' data: https:; \
                 connect-src 'self'; \
                 frame-ancestors 'none'; \
                 base-uri 'self'; \
                 form-action 'self';",
            ),
        ))
        .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BODY_SIZE))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(DefaultOnRequest::new().level(Level::DEBUG))
                .on_response(DefaultOnResponse::new().level(Level::DEBUG)),
        )
        .with_state(state)
}
