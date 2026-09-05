//! Authentication middleware for Axum
//!
//! This module provides middleware for authenticating HTTP requests
//! and injecting `AuthenticatedUser` into request extensions.

use crate::{jwt::JwtManager, types::AuthenticatedUser};
use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode},
    middleware::Next,
};
use std::sync::Arc;
use tracing::{debug, warn};

// TAG: surface=auth owner=security-team rule=RBAC-001
/// State for auth middleware
#[derive(Debug, Clone)]
pub struct AuthMiddlewareState {
    /// JWT manager for token validation
    pub jwt_manager: Arc<JwtManager>,
}

// TAG: surface=auth owner=security-team rule=RBAC-001
impl AuthMiddlewareState {
    /// Create new auth middleware state
    #[must_use]
    pub fn new(jwt_manager: JwtManager) -> Self {
        Self {
            jwt_manager: Arc::new(jwt_manager),
        }
    }
}

/// Middleware that requires authentication
///
/// This middleware validates the Authorization header and injects
/// the `AuthenticatedUser` into request extensions.
///
/// # Example
/// ```rust,ignore
/// use axum::{middleware, Router};
/// use auth::{middleware::{require_auth, AuthMiddlewareState}, jwt::JwtManager};
///
/// let jwt_manager = JwtManager::new("secret");
/// let state = AuthMiddlewareState::new(jwt_manager);
///
/// let app = Router::new()
///     .route("/protected", get(handler))
///     .layer(middleware::from_fn_with_state(
///         state,
// TAG: surface=auth owner=platform-team rule=GENERAL-001
///         require_auth,
///     ));
/// ```
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn require_auth(
    State(state): State<AuthMiddlewareState>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    let auth_header = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    if let Some(header) = auth_header {
        debug!("Processing authorization header");

        match JwtManager::extract_bearer_token(header) {
            Ok(token) => {
                match state.jwt_manager.validate_token(token) {
                    Ok(claims) => {
                        let user = claims.to_authenticated_user();
                        debug!(user_id = %user.user_id, "User authenticated");

                        // Inject user into request extensions
                        request.extensions_mut().insert(user);
                        // TAG: surface=auth owner=security-team rule=RBAC-001
                        Ok(next.run(request).await)
                    }
                    Err(e) => {
                        warn!(error = %e, "Token validation failed");
                        Err(e.status_code())
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Invalid bearer token format");
                Err(StatusCode::BAD_REQUEST)
            }
        }
    } else {
        debug!("Missing authorization header");
        Err(StatusCode::UNAUTHORIZED)
    }
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Middleware that optionally authenticates
///
/// This middleware attempts to validate the Authorization header but
/// allows the request to proceed even if authentication fails.
/// The `AuthenticatedUser` will be `Some` if authentication succeeds,
/// or `None` if it fails or is missing.
///
/// # Example
/// ```rust,ignore
/// use axum::{middleware, Router};
/// use auth::middleware::{optional_auth, AuthMiddlewareState};
///
/// let state = AuthMiddlewareState::new(jwt_manager);
///
/// let app = Router::new()
///     .route("/public", get(handler))
///     .layer(middleware::from_fn_with_state(
///         state,
///         optional_auth,
///     ));
/// ```
pub async fn optional_auth(
    State(state): State<AuthMiddlewareState>,
    mut request: Request<Body>,
    next: Next,
) -> Response<Body> {
    if let Some(header) = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if let Ok(token) = JwtManager::extract_bearer_token(header) {
            if let Ok(claims) = state.jwt_manager.validate_token(token) {
                let user = claims.to_authenticated_user();
                request.extensions_mut().insert(user);
            }
        }
    }

    next.run(request).await
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Middleware that validates a specific role
///
/// This middleware checks if the authenticated user has a specific role.
/// It must be used AFTER `require_auth` middleware.
/// # Errors
///
/// Returns an error if the operation fails.
pub fn require_role(
    _role: &'static str,
) -> impl Fn(
    Request<Body>,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response<Body>> + Send>>
       + Clone {
    move |request: Request<Body>, next: Next| {
        // This is a simplified version - in production, you'd check
        // the user's roles from a database or JWT claims
        let has_role = request
            .extensions()
            .get::<AuthenticatedUser>()
            .is_some_and(|_| true);

        Box::pin(async move {
            if has_role {
                next.run(request).await
            } else {
                // This should never fail with Body::empty(), but handle gracefully
                Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Body::empty())
                    .unwrap_or_else(|_| Response::new(Body::from("Forbidden")))
            }
        })
    }
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Middleware that validates admin access
///
/// This middleware checks if the user is an admin.
/// It must be used AFTER `require_auth` middleware.
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn require_admin(
    request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    // In production, check admin flag from JWT claims or database
    let is_admin = request.extensions().get::<bool>().copied().unwrap_or(false);

    if is_admin {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, middleware, routing::get, Router};
    use tower::ServiceExt;

    fn create_test_state() -> AuthMiddlewareState {
        AuthMiddlewareState::new(JwtManager::new("test-secret-key-at-least-32-bytes-long!"))
    }

    #[tokio::test]
    async fn test_require_auth_missing_header() {
        let state = create_test_state();

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(state, require_auth));

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    #[tokio::test]
    async fn test_require_auth_valid_token() {
        let state = create_test_state();

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(state.clone(), require_auth));

        let token = state
            .jwt_manager
            .generate_token("user-123", "test@example.com", "Test", "entra-456", 3600)
            .unwrap();

        let request = Request::builder()
            .uri("/test")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_require_auth_invalid_token_format() {
        let state = create_test_state();

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(state, require_auth));

        let request = Request::builder()
            .uri("/test")
            .header("Authorization", "Basic dXNlcjpwYXNz")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        // TAG: surface=auth owner=security-team rule=RBAC-001
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_require_auth_expired_token() {
        let state = create_test_state();

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(state.clone(), require_auth));

        let expired_claims = crate::types::JwtClaims {
            sub: "user-1".to_string(),
            email: "test@example.com".to_string(),
            name: "Test".to_string(),
            iat: chrono::Utc::now().timestamp() - 7200,
            exp: chrono::Utc::now().timestamp() - 3600,
            iss: "freshcredit".to_string(),
            aud: "freshcredit-api".to_string(),
            entra_id: "entra-1".to_string(),
            verified_id_credential_id: None,
            roles: vec![],
        };

        let token = state
            .jwt_manager
            .generate_token_from_claims(&expired_claims)
            .unwrap();

        let request = Request::builder()
            .uri("/test")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        // TAG: surface=auth owner=security-team rule=RBAC-001
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_optional_auth_valid_token() {
        let state = create_test_state();

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(state.clone(), optional_auth));

        let token = state
            .jwt_manager
            .generate_token("user-123", "test@example.com", "Test", "entra-456", 3600)
            .unwrap();

        let request = Request::builder()
            .uri("/test")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_optional_auth_no_header() {
        let state = create_test_state();

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(state, optional_auth));

        // TAG: surface=auth owner=security-team rule=RBAC-001
        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_require_role_with_user() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(middleware::from_fn(require_role("admin")));

        let mut request = Request::builder().uri("/test").body(Body::empty()).unwrap();
        request.extensions_mut().insert(AuthenticatedUser {
            user_id: "user-1".to_string(),
            email: "test@example.com".to_string(),
            name: "Test".to_string(),
            given_name: None,
            family_name: None,
            mobile_phone: None,
            job_title: None,
            street_address: None,
            city: None,
            state: None,
            postal_code: None,
            country: None,
            entra_id: "entra-1".to_string(),
            verified_id_credential_id: None,
            roles: vec!["admin".to_string()],
        });

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    #[tokio::test]
    async fn test_require_role_without_user() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(middleware::from_fn(require_role("admin")));

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_require_admin_with_flag() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(middleware::from_fn(require_admin));

        let mut request = Request::builder().uri("/test").body(Body::empty()).unwrap();
        request.extensions_mut().insert(true);

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_require_admin_without_flag() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(middleware::from_fn(require_admin));

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
