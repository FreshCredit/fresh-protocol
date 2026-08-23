//! Shared helper functions.

use crate::config::{API_KEY, CORS_MAX_AGE_SECS, JWT_SECRET, RATE_LIMIT_REQUESTS};
use crate::node::AppState;
use axum::{
    extract::{Request, State},
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{Json, Response},
};
use governor::middleware::StateInformationMiddleware;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{info, warn};

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Extract and validate JWT from Authorization header
pub fn extract_jwt(auth_header: &str) -> Option<&str> {
    auth_header.strip_prefix("Bearer ")
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Validate JWT token and return claims
pub fn validate_jwt(token: &str) -> Result<crate::config::Claims, jsonwebtoken::errors::Error> {
    let validation = Validation::new(Algorithm::HS256);
    decode::<crate::config::Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
}

/// Validate API key (constant-time comparison to prevent timing attacks)
pub fn validate_api_key(api_key: &str) -> bool {
    constant_time_compare(api_key, &API_KEY)
}

/// Constant-time string comparison to prevent timing attacks
pub fn constant_time_compare(a: &str, b: &str) -> bool {
    use std::iter::zip;

    if a.len() != b.len() {
        return false;
    }

    zip(a.bytes(), b.bytes())
        .map(|(a, b)| a ^ b)
        .fold(0u8, |acc, x| acc | x)
        == 0
}

/// JWT Authentication middleware
// TAG: surface=blockchain owner=platform-team rule=GENERAL-001
pub async fn jwt_auth_middleware(
    State(_state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<crate::config::AuthError>)> {
    // Extract Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(crate::config::AuthError {
                    error: "unauthorized".to_string(),
                    message: "Missing Authorization header".to_string(),
                }),
            )
        })?;

    // Extract Bearer token
    let token = extract_jwt(auth_header).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(crate::config::AuthError {
                error: "unauthorized".to_string(),
                message: "Invalid Authorization header format. Use: Bearer <token>".to_string(),
            }),
        )
    })?;

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    // Validate JWT
    let claims = validate_jwt(token).map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            Json(crate::config::AuthError {
                error: "unauthorized".to_string(),
                message: format!("Invalid token: {e}"),
            }),
        )
    })?;

    // Check expiration
    let now = chrono::Utc::now().timestamp();
    if claims.exp < now {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(crate::config::AuthError {
                error: "unauthorized".to_string(),
                message: "Token expired".to_string(),
            }),
        ));
    }

    // Store claims in request extensions for handlers to access
    let mut request = request;
    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// API Key Authentication middleware
pub async fn api_key_auth_middleware(
    State(_state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<crate::config::AuthError>)> {
    // Extract X-API-Key header
    let api_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(crate::config::AuthError {
                    error: "unauthorized".to_string(),
                    message: "Missing X-API-Key header".to_string(),
                }),
            )
        })?;

    // Validate API key (constant-time comparison)
    if !validate_api_key(api_key) {
        warn!("Invalid API key attempt");
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(crate::config::AuthError {
                error: "unauthorized".to_string(),
                message: "Invalid API key".to_string(),
            }),
        ));
    }

    Ok(next.run(request).await)
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Combined auth middleware - tries API key first, then JWT
pub async fn combined_auth_middleware(
    state: State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<crate::config::AuthError>)> {
    // Try API Key first
    if request.headers().contains_key("X-API-Key") {
        return api_key_auth_middleware(state, request, next).await;
    }

    // Fall back to JWT
    jwt_auth_middleware(state, request, next).await
}

/// Optional auth middleware - doesn't fail if no auth provided
/// Used for endpoints that work with or without authentication
#[allow(dead_code)]
pub async fn optional_auth_middleware(
    _state: State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    // Try to extract and validate JWT if present
    if let Some(auth_header) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if let Some(token) = extract_jwt(auth_header) {
            if let Ok(claims) = validate_jwt(token) {
                let now = chrono::Utc::now().timestamp();
                if claims.exp >= now {
                    request.extensions_mut().insert(claims);
                }
            }
        }
    }

    next.run(request).await
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Wait for shutdown signal (Ctrl+C or SIGTERM)
/// P0-BEST-PRACTICES: Handles graceful shutdown for Cloud Run and Kubernetes
pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown...");
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Returns allowed CORS origins based on environment
/// `HARDCODED_URL`: CORS allowed origins - production and testnet domains
pub fn allowed_origins() -> Vec<&'static str> {
    // Check if running in production (Cloud Run sets K_SERVICE env var)
    if std::env::var("K_SERVICE").is_ok() {
        vec![
            "https://freshcredit.com",
            "https://app.freshcredit.com",
            "https://api.freshcredit.com",
            "https://blockchain.freshcredit.com",
            "https://testnet.freshcredit.com",
        ]
    } else {
        vec![
            "http://localhost:3000",
            "http://localhost:3001",
            "http://localhost:9933",
            "http://127.0.0.1:3000",
            "http://127.0.0.1:3001",
            "http://127.0.0.1:9933",
        ]
    }
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
pub fn build_cors_layer() -> CorsLayer {
    let origins: Vec<_> = allowed_origins()
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
        .allow_credentials(true)
        .max_age(std::time::Duration::from_secs(CORS_MAX_AGE_SECS.into()))
}

/// Build rate limiting layer
///
/// Limits requests to `RATE_LIMIT_REQUESTS` per `RATE_LIMIT_WINDOW_SECS` per IP
pub fn build_rate_limit_layer() -> GovernorLayer<SmartIpKeyExtractor, StateInformationMiddleware> {
    let config = std::sync::Arc::new(
        GovernorConfigBuilder::default()
            .per_second(1)
            .burst_size(u32::try_from(RATE_LIMIT_REQUESTS).unwrap_or(u32::MAX))
            .use_headers() // Enable X-RateLimit headers
            .key_extractor(SmartIpKeyExtractor) // Use client IP as key
            .finish()
            .expect("valid governor config"),
    );

    // TAG: surface=blockchain owner=platform-team rule=GENERAL-001
    GovernorLayer { config }
}

/// Store report data in the NOMT sidecar at anchor time (fire-and-forget) so
/// `/proof/:hash` can serve edge-first proofs. The stored bytes must be
/// exactly what `create_hash` anchors — the sidecar keys entries on
/// `SHA256(data)`, and `create_hash` anchors `SHA256(serde_json::to_string(...))`
/// of the same value — so the on-chain hash equals the sidecar key.
/// Best-effort: logs on failure, never fails the anchor.
pub fn spawn_nomt_store(
    client: &reqwest::Client,
    nomt_url: &str,
    user_id: String,
    report_type: String,
    data: Vec<u8>,
) {
    let url = format!("{}/store", nomt_url.trim_end_matches('/'));
    let client = client.clone();
    tokio::spawn(async move {
        let body = serde_json::json!({
            "user_id": user_id,
            "report_type": report_type,
            "data": data,
        });
        match client.post(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("[NOMT] stored report hash via sidecar /store");
            }
            Ok(resp) => {
                tracing::warn!("[NOMT] sidecar /store returned {}", resp.status());
            }
            Err(e) => {
                tracing::warn!("[NOMT] sidecar /store failed (anchor unaffected): {e}");
            }
        }
    });
}

/// Verify hash via NOMT sidecar HTTP API
pub async fn verify_hash_via_nomt(
    client: &reqwest::Client,
    nomt_url: &str,
    hash: &str,
) -> Result<Option<crate::config::NomtVerifyResult>, anyhow::Error> {
    let url = format!("{}/verify/{}", nomt_url.trim_end_matches('/'), hash);
    let response = client.get(&url).send().await?;

    if response.status().is_success() {
        let body: serde_json::Value = response.json().await?;
        if body
            .get("verified")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            Ok(Some(crate::config::NomtVerifyResult {
                root: body
                    .get("root")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                proof_available: body
                    .get("proof_available")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
                timestamp: body
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            }))
        } else {
            Ok(None)
        }
    } else if response.status().as_u16() == 404 {
        Ok(None)
    } else {
        Err(anyhow::anyhow!(
            "NOMT sidecar returned error: {}",
            response.status()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_jwt_valid() {
        assert_eq!(extract_jwt("Bearer token123"), Some("token123"));
    }

    #[test]
    fn test_extract_jwt_invalid() {
        assert_eq!(extract_jwt("Basic token123"), None);
        assert_eq!(extract_jwt("token123"), None);
    }

    #[test]
    fn test_constant_time_compare_wrong_length() {
        assert!(!constant_time_compare("short", "longer_key"));
        assert!(!constant_time_compare("same_len1", "same_len2"));
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    #[test]
    fn test_constant_time_compare_correct() {
        assert!(constant_time_compare("secret123", "secret123"));
    }

    #[test]
    fn test_constant_time_compare_incorrect_same_length() {
        assert!(!constant_time_compare("secret123", "secret124"));
    }

    #[test]
    #[serial_test::serial]
    fn test_allowed_origins_production() {
        std::env::set_var("K_SERVICE", "some-service");
        let origins = allowed_origins();
        assert!(origins.contains(&"https://freshcredit.com"));
        assert!(origins.contains(&"https://app.freshcredit.com"));
        std::env::remove_var("K_SERVICE");
    }

    #[test]
    fn test_allowed_origins_local() {
        std::env::remove_var("K_SERVICE");
        let origins = allowed_origins();
        assert!(origins.contains(&"http://localhost:3000"));
        assert!(origins.contains(&"http://127.0.0.1:3000"));
    }

    #[test]
    fn test_build_cors_layer() {
        std::env::remove_var("K_SERVICE");
        let layer = build_cors_layer();
        // Just verify it builds without panicking
        let _ = layer;
    }
}
