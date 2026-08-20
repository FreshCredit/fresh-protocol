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
    store::{IdempotencyError, IdempotencyStore},
};
use freshcredit_core_timing::SystemClock;

/// Idempotency layer for Axum
#[derive(Clone, Debug)]
pub struct IdempotencyLayer {
    store: Arc<IdempotencyStore>,
    config: IdempotencyConfig,
}

impl IdempotencyLayer {
    /// Create a new idempotency layer
    #[must_use]
    pub fn new(config: IdempotencyConfig) -> Self {
        let store = Arc::new(IdempotencyStore::new(config.clone(), Arc::new(SystemClock)));

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

        // Check if we have a stored response for this key
        if let Some(stored) = self.store.get(&key) {
            info!("Returning cached response for idempotency key: {}", key);
            return Self::build_response_from_stored(stored);
        }

        // Process the request
        debug!("Processing new request with idempotency key: {}", key);
        let response = next.run(req).await;

        // Store the response if it's a success (2xx or 3xx)
        let status = response.status();
        if status.is_success() || status.is_redirection() {
            // Store the response and return the stored version
            match self.store_response(&key, response).await {
                Ok(stored) => {
                    info!("Stored response for idempotency key: {}", key);
                    return Self::build_response_from_stored(stored);
                }
                Err(e) => {
                    warn!("Failed to store idempotency response: {}", e);
                    return Self::build_error_response();
                }
            }
        }

        response
    }

    /// Store a response with the idempotency key and return the stored response
    async fn store_response(
        &self,
        key: &str,
        // TAG: surface=security owner=platform-team rule=MID-001
        response: Response,
    ) -> Result<crate::store::StoredResponse, IdempotencyError> {
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
            .store(key, parts.status.as_u16(), headers, body_bytes)?;

        // Return the stored response
        self.store.get(key).ok_or_else(|| {
            IdempotencyError::SerializationError("Failed to retrieve stored response".to_string())
        })
    }

    /// Build a response from stored data
    fn build_response_from_stored(stored: crate::store::StoredResponse) -> Response {
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
        let response = IdempotencyLayer::build_error_response();
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
        };
        let response = IdempotencyLayer::build_response_from_stored(stored);
        assert_eq!(response.status(), StatusCode::OK);
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
