// TAG: surface=security owner=security-team rule=SEC-001
//! Axum middleware for idempotency key enforcement

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};

use crate::{
    config::IdempotencyConfig,
    store::{IdempotencyError, LookupOutcome, ResponseStore, StoredResponse},
};
use freshcredit_core_timing::SystemClock;

/// Idempotency layer for Axum
///
/// Generic over the [`ResponseStore`] backend: defaults to the in-memory
/// [`crate::IdempotencyStore`]; pass a [`crate::DurableIdempotencyStore`] via
/// [`IdempotencyLayer::with_store`] for at-most-once execution across
/// replicas.
pub struct IdempotencyLayer<S = crate::IdempotencyStore> {
    store: Arc<S>,
    config: IdempotencyConfig,
}

impl<S> Clone for IdempotencyLayer<S> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            config: self.config.clone(),
        }
    }
}

impl<S> std::fmt::Debug for IdempotencyLayer<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdempotencyLayer")
            .field("config", &self.config)
            .field("store", &"<store>")
            .finish_non_exhaustive()
    }
}

impl IdempotencyLayer<crate::IdempotencyStore> {
    /// Create a new idempotency layer backed by the in-memory store
    #[must_use]
    pub fn new(config: IdempotencyConfig) -> Self {
        let store = Arc::new(crate::IdempotencyStore::new(
            config.clone(),
            Arc::new(SystemClock),
        ));

        Self { store, config }
    }

    /// Create idempotency layer with default configuration (24 hour TTL)
    #[must_use]
    // TAG: surface=security owner=platform-team rule=MID-001
    pub fn default_ttl() -> Self {
        Self::new(IdempotencyConfig::default())
    }

    /// Create idempotency layer with required idempotency key
    #[must_use]
    pub fn required(ttl: std::time::Duration) -> Self {
        Self::new(IdempotencyConfig::required(ttl))
    }
}

impl<S: ResponseStore> IdempotencyLayer<S> {
    /// Create an idempotency layer with an explicit store backend
    #[must_use]
    pub const fn with_store(config: IdempotencyConfig, store: Arc<S>) -> Self {
        Self { store, config }
    }

    /// Middleware handler
    #[instrument(name = "idempotency_middleware", skip(self, req, next), fields(method = %req.method(), path = %req.uri().path()))]
    pub async fn handle(&self, req: Request, next: Next) -> Response {
        // Only apply idempotency to POST, PUT, PATCH, DELETE
        let method = req.method().clone();
        if !matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE") {
            return next.run(req).await;
        }

        // Extract idempotency key from header
        let idempotency_key = req
            .headers()
            .get(&self.config.header_name)
            .and_then(|v| v.to_str().ok())
            .map(std::string::ToString::to_string);

        // Check if idempotency key is required
        if self.config.required && idempotency_key.is_none() {
            warn!("Idempotency key required but not provided");
            return (
                StatusCode::BAD_REQUEST,
                "Idempotency-Key header is required for this endpoint",
            )
                .into_response();
        }

        // If no idempotency key provided (and not required), proceed normally
        // TAG: surface=security owner=platform-team rule=GENERAL-001
        let Some(key) = idempotency_key else {
            return next.run(req).await;
        };

        // Extract principal/credential scope so Idempotency-Keys can NEVER leak across users
        let principal_scope = if let Some(auth_hdr) = req
            .headers()
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
        {
            format!("auth:{auth_hdr}")
        } else if let Some(api_key) = req.headers().get("x-api-key").and_then(|v| v.to_str().ok()) {
            format!("apikey:{api_key}")
        } else if let Some(cookie) = req
            .headers()
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
        {
            format!("cookie:{cookie}")
        } else {
            "anon".to_string()
        };

        let uri_path = req.uri().path().to_string();
        let (parts, req_body) = req.into_parts();
        let req_bytes = match req_body.collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => {
                warn!("Failed to read request body for idempotency check: {e}");
                return (StatusCode::BAD_REQUEST, "Invalid request body").into_response();
            }
        };
        if req_bytes.len() > self.config.max_body_size {
            return (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large").into_response();
        }

        let (scoped_key, request_hash) = {
            use sha2::{Digest, Sha256};
            let mut ks = Sha256::new();
            ks.update(principal_scope.as_bytes());
            ks.update(b"\x00");
            ks.update(method.as_str().as_bytes());
            ks.update(b"\x00");
            ks.update(uri_path.as_bytes());
            ks.update(b"\x00");
            ks.update(key.as_bytes());

            let mut rf = Sha256::new();
            rf.update(method.as_str().as_bytes());
            rf.update(b"\x00");
            rf.update(uri_path.as_bytes());
            rf.update(b"\x00");
            rf.update(&req_bytes);
            (hex::encode(ks.finalize()), hex::encode(rf.finalize()))
        };

        let req = Request::from_parts(parts, Body::from(req_bytes));

        // Decide whether to replay, proceed, or conflict.
        match self.store.lookup(&scoped_key).await {
            Ok(LookupOutcome::Replay(stored)) => {
                if let Some(ref prev_hash) = stored.request_hash {
                    if prev_hash != &request_hash {
                        warn!("Idempotency key reused with mismatched request payload");
                        return (
                            StatusCode::UNPROCESSABLE_ENTITY,
                            "Idempotency-Key already used with a different request payload",
                        )
                            .into_response();
                    }
                }
                info!("Returning cached response for idempotency key");
                let mut response = Self::build_response_from_stored(stored);
                if let Some(header_name) = &self.config.replay_header {
                    if let Ok(name) = HeaderName::try_from(header_name.clone()) {
                        response
                            .headers_mut()
                            .insert(name, HeaderValue::from_static("true"));
                    }
                }
                response
            }
            Ok(LookupOutcome::Conflict) => {
                warn!("Concurrent request with same idempotency key");
                StatusCode::CONFLICT.into_response()
            }
            Ok(LookupOutcome::Proceed) => {
                // Process the request
                debug!("Processing new request with scoped idempotency key");
                let response = next.run(req).await;

                // Store the response if it's a success (2xx or 3xx)
                let status = response.status();
                if status.is_success() || status.is_redirection() {
                    match self
                        .store_response_with_fingerprint(&scoped_key, response, Some(request_hash))
                        .await
                    {
                        Ok(stored) => {
                            info!("Stored response for scoped idempotency key");
                            Self::build_response_from_stored(stored)
                        }
                        Err(e) => {
                            warn!("Failed to store idempotency response: {}", e);
                            Self::build_error_response()
                        }
                    }
                } else {
                    response
                }
            }
            Err(e) => {
                // Fail closed: never execute when the store cannot be trusted.
                warn!("Idempotency store lookup failed: {}", e);
                Self::build_error_response()
            }
        }
    }

    /// Store a response with the idempotency key and request fingerprint
    async fn store_response_with_fingerprint(
        &self,
        key: &str,
        // TAG: surface=security owner=platform-team rule=MID-001
        response: Response,
        request_hash: Option<String>,
    ) -> Result<StoredResponse, IdempotencyError> {
        let (parts, body) = response.into_parts();

        // Collect the body
        let body_bytes = body
            .collect()
            .await
            .map_err(|e| IdempotencyError::SerializationError(e.to_string()))?
            .to_bytes()
            .to_vec();

        // Extract headers
        let headers: Vec<(String, String)> = parts
            .headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|v| (name.to_string(), v.to_string()))
            })
            .collect();

        // Store the response
        self.store
            .store_response_with_fingerprint(
                key,
                parts.status.as_u16(),
                headers,
                body_bytes,
                request_hash,
            )
            .await
    }
}

impl<S> IdempotencyLayer<S> {
    /// Build a response from stored data
    fn build_response_from_stored(stored: StoredResponse) -> Response {
        let mut response = Response::builder().status(stored.status);

        // TAG: surface=security owner=security-team rule=SEC-001
        // Add headers
        if let Some(headers) = response.headers_mut() {
            for (name, value) in stored.headers {
                if let (Ok(header_name), Ok(header_value)) =
                    (HeaderName::try_from(name), HeaderValue::try_from(value))
                {
                    headers.insert(header_name, header_value);
                }
            }
        }

        response
            .body(Body::from(stored.body))
            .unwrap_or_else(|_| Self::build_error_response())
    }

    /// Build an error response
    fn build_error_response() -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to process idempotency response",
        )
            .into_response()
    }
}

#[cfg(test)]
#[allow(unused_variables)]
mod tests {
    use super::*;

    #[test]
    fn test_idempotency_layer_new() {
        let layer =
            IdempotencyLayer::new(IdempotencyConfig::new(std::time::Duration::from_secs(3600)));
        assert_eq!(layer.config.ttl, std::time::Duration::from_secs(3600));
    }
    // TAG: surface=security owner=platform-team rule=MID-001

    #[test]
    fn test_idempotency_layer_default_ttl() {
        let layer = IdempotencyLayer::default_ttl();
        assert_eq!(
            layer.config.ttl,
            std::time::Duration::from_secs(24 * 60 * 60)
        );
    }

    #[test]
    fn test_idempotency_layer_required() {
        let layer = IdempotencyLayer::required(std::time::Duration::from_secs(1800));
        assert!(layer.config.required);
        assert_eq!(layer.config.ttl, std::time::Duration::from_secs(1800));
    }

    #[test]
    fn test_build_error_response() {
        let layer = IdempotencyLayer::default_ttl();
        let response = IdempotencyLayer::<crate::IdempotencyStore>::build_error_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_build_response_from_stored() {
        let layer = IdempotencyLayer::default_ttl();
        let stored = crate::store::StoredResponse {
            status: 200,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: b"{\"ok\": true}".to_vec(),
            stored_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            request_hash: None,
        };
        let response =
            IdempotencyLayer::<crate::IdempotencyStore>::build_response_from_stored(stored);
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_idempotency_key_isolated_across_users_and_rejects_body_mismatch() {
        use axum::{routing::post, Router};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tower::ServiceExt;

        let counter = Arc::new(AtomicUsize::new(0));
        let layer = IdempotencyLayer::new(
            IdempotencyConfig::new(std::time::Duration::from_secs(60))
                .with_replay_header("X-Idempotent-Replay".to_string()),
        );
        let counter_cloned = Arc::clone(&counter);
        let app = Router::new()
            .route(
                "/pay",
                post(move |body: String| {
                    let c = Arc::clone(&counter_cloned);
                    async move {
                        let n = c.fetch_add(1, Ordering::SeqCst) + 1;
                        format!("exec-{n}:{body}")
                    }
                }),
            )
            .layer(axum::middleware::from_fn(move |req, next| {
                let l = layer.clone();
                async move { l.handle(req, next).await }
            }));

        // User A executes with key K1 and body {"amt":10}
        let resp_a1 = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pay")
                    .header("Authorization", "Bearer token-alice")
                    .header("Idempotency-Key", "K1")
                    .body(Body::from("{\"amt\":10}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp_a1.status(), StatusCode::OK);
        let bytes_a1 = resp_a1.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&bytes_a1[..], b"exec-1:{\"amt\":10}");

        // User B sending the SAME Idempotency-Key K1 must NOT receive Alice's cached response
        let resp_b = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pay")
                    .header("Authorization", "Bearer token-bob")
                    .header("Idempotency-Key", "K1")
                    .body(Body::from("{\"amt\":10}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp_b.status(), StatusCode::OK);
        assert!(resp_b.headers().get("X-Idempotent-Replay").is_none());
        let bytes_b = resp_b.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&bytes_b[..], b"exec-2:{\"amt\":10}");

        // Alice replaying K1 with the SAME payload gets cached exec-1
        let resp_a_replay = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pay")
                    .header("Authorization", "Bearer token-alice")
                    .header("Idempotency-Key", "K1")
                    .body(Body::from("{\"amt\":10}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp_a_replay.status(), StatusCode::OK);
        assert_eq!(
            resp_a_replay
                .headers()
                .get("X-Idempotent-Replay")
                .unwrap()
                .to_str()
                .unwrap(),
            "true"
        );
        let bytes_a_rep = resp_a_replay
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        assert_eq!(&bytes_a_rep[..], b"exec-1:{\"amt\":10}");

        // Alice reusing K1 with a DIFFERENT payload is rejected with 422 Unprocessable Entity
        let resp_a_mismatch = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/pay")
                    .header("Authorization", "Bearer token-alice")
                    .header("Idempotency-Key", "K1")
                    .body(Body::from("{\"amt\":999}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp_a_mismatch.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
