//! Axum extractors for authentication
//!
//! This module provides `FromRequestParts` implementations for extracting
//! authenticated users from HTTP requests in Axum applications.

use crate::types::{AuthError, AuthenticatedUser, JwtClaims};
use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use std::sync::Arc;

// TAG: surface=auth owner=security-team rule=RBAC-001
// TAG: surface=auth owner=security-team rule=RBAC-001
/// Extractor that pulls the authenticated user from request extensions
///
/// This extractor assumes that authentication middleware has already run
/// and populated the `AuthenticatedUser` in request extensions.
///
/// # Example
/// ```rust,ignore
/// use axum::{routing::get, Router};
/// use auth::extractors::AuthExtractor;
///
/// async fn handler(AuthExtractor(user): AuthExtractor) -> String {
///     format!("Hello, {}!", user.name)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AuthExtractor(pub AuthenticatedUser);

#[async_trait]
impl<S> FromRequestParts<S> for AuthExtractor
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthenticatedUser>()
            .cloned()
            .map(AuthExtractor)
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Extractor that optionally pulls the authenticated user
///
/// Returns `None` if no user is authenticated, allowing handlers
/// to work with both authenticated and unauthenticated requests.
///
/// # Example
/// ```rust,ignore
/// use axum::{routing::get, Router};
/// use auth::extractors::OptionalAuthExtractor;
///
/// async fn handler(
///     OptionalAuthExtractor(user): OptionalAuthExtractor,
/// ) -> String {
///     match user {
///         Some(u) => format!("Hello, {}!", u.name),
///         None => "Hello, guest!".to_string(),
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct OptionalAuthExtractor(pub Option<AuthenticatedUser>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalAuthExtractor
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // TAG: surface=auth owner=platform-team rule=GENERAL-001
        let user = parts.extensions.get::<AuthenticatedUser>().cloned();
        Ok(Self(user))
    }
}

/// Extractor for admin users only
///
/// Returns 403 Forbidden if the user is not an admin.
#[derive(Debug, Clone)]
pub struct AdminExtractor(pub AuthenticatedUser);

#[async_trait]
impl<S> FromRequestParts<S> for AdminExtractor
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthExtractor(user) = AuthExtractor::from_request_parts(parts, state).await?;

        // Check if user has admin role (admin or internal role)
        if user.is_admin() {
            Ok(Self(user))
        } else {
            Err(StatusCode::FORBIDDEN)
        }
    }
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Extractor that validates JWT from Authorization header
///
/// This extractor performs full JWT validation on each request.
/// For better performance, use middleware to validate once and
/// `AuthExtractor` to retrieve the user.
#[derive(Debug, Clone)]
pub struct JwtExtractor {
    /// The authenticated user
    pub user: AuthenticatedUser,
    /// Raw JWT claims
    pub claims: JwtClaims,
    /// Raw token string
    pub token: String,
}

/// State required for JWT extraction
#[derive(Debug, Clone)]
pub struct JwtExtractorState {
    /// JWT secret key
    pub secret: String,
}

// TAG: surface=auth owner=security-team rule=RBAC-001
#[async_trait]
impl FromRequestParts<Arc<JwtExtractorState>> for JwtExtractor {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<JwtExtractorState>,
    ) -> Result<Self, Self::Rejection> {
        use crate::jwt::JwtManager;

        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let token =
            JwtManager::extract_bearer_token(auth_header).map_err(|_| StatusCode::UNAUTHORIZED)?;

        let manager = JwtManager::new(&state.secret);
        let claims = manager.validate_token(token).map_err(|e| match e {
            AuthError::TokenExpired => StatusCode::UNAUTHORIZED,
            AuthError::InvalidSignature => StatusCode::UNAUTHORIZED,
            _ => StatusCode::BAD_REQUEST,
        })?;

        let user = claims.to_authenticated_user();

        Ok(Self {
            user,
            claims,
            token: token.to_string(),
        })
    }
}

// TAG: surface=auth owner=security-team rule=RBAC-001
/// Extension trait for adding auth to request parts
pub trait AuthExtension {
    /// Insert an authenticated user into extensions
    #[must_use]
    fn with_user(self, user: AuthenticatedUser) -> Self;
}

impl AuthExtension for axum::http::Extensions {
    fn with_user(mut self, user: AuthenticatedUser) -> Self {
        self.insert(user);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    fn create_test_user() -> AuthenticatedUser {
        AuthenticatedUser::new("user-123", "test@example.com", "Test User", "entra-456")
    }

    #[tokio::test]
    async fn test_auth_extractor_success() {
        let user = create_test_user();
        // TAG: surface=auth owner=platform-team rule=GENERAL-001
        let mut parts = Request::builder()
            .uri("/test")
            .body(())
            .unwrap()
            .into_parts()
            .0;

        parts.extensions.insert(user.clone());

        let result = AuthExtractor::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0.user_id, user.user_id);
    }

    #[tokio::test]
    async fn test_auth_extractor_missing_user() {
        let mut parts = Request::builder()
            .uri("/test")
            .body(())
            .unwrap()
            .into_parts()
            .0;

        let result = AuthExtractor::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }

    // TAG: surface=auth owner=security-team rule=RBAC-001
    #[tokio::test]
    async fn test_optional_auth_extractor_with_user() {
        let user = create_test_user();
        let mut parts = Request::builder()
            .uri("/test")
            .body(())
            .unwrap()
            .into_parts()
            .0;

        parts.extensions.insert(user.clone());

        let result = OptionalAuthExtractor::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().0.is_some());
    }

    #[tokio::test]
    async fn test_optional_auth_extractor_without_user() {
        let mut parts = Request::builder()
            .uri("/test")
            .body(())
            .unwrap()
            .into_parts()
            .0;

        let result = OptionalAuthExtractor::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().0.is_none());
    }
}
