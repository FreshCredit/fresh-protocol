// TAG: surface=security owner=security-team rule=SEC-001
//! Axum session middleware

use crate::config::{SameSitePolicy, SessionConfig};
use crate::storage::{Session, SessionError, SessionStorage};
use axum::http::request::Parts;
use axum::{
    extract::{FromRequestParts, Request},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::{debug, warn};

/// Session middleware state
#[derive(Clone, Debug)]
pub struct SessionMiddleware<S: SessionStorage> {
    storage: Arc<S>,
    config: SessionConfig,
}

impl<S: SessionStorage> SessionMiddleware<S> {
    /// Create a new session middleware
    pub const fn new(storage: Arc<S>, config: SessionConfig) -> Self {
        Self { storage, config }
    }

    /// Validate and refresh session
    /// P0-FIX: Uses atomic get-and-refresh to prevent TOCTOU race condition
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    // TAG: surface=security owner=platform-team rule=MID-001
    pub async fn validate_session(&self, session_id: &str) -> Result<Session, SessionError> {
        // Try to atomically get and refresh the session
        match self
            .storage
            .get_and_refresh(session_id, self.config.timeout)
            .await?
        {
            Some(session) => {
                // Check if expired after refresh
                if session.is_expired() {
                    let _ = self.storage.delete(session_id).await; // AUDIT-OK(fire-and-forget): best-effort cleanup of expired session; session already unusable
                    return Err(SessionError::Expired);
                }
                // Check max age
                if session.is_max_age_exceeded(self.config.max_age) {
                    let _ = self.storage.delete(session_id).await; // AUDIT-OK(fire-and-forget): best-effort cleanup of over-max-age session
                    return Err(SessionError::MaxAgeExceeded);
                }
                Ok(session)
            }
            None => Err(SessionError::NotFound),
        }
    }

    /// Create a new session for a user
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn create_session(&self, user_id: String) -> Result<Session, SessionError> {
        let session = Session::new(user_id, self.config.timeout, self.config.max_age);
        self.storage.create(session.clone()).await?;
        Ok(session)
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001

    /// Delete a session
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn delete_session(&self, session_id: &str) -> Result<(), SessionError> {
        self.storage.delete(session_id).await
    }
}

/// Session extractor for Axum handlers
#[derive(Debug)]
pub struct SessionExtractor {
    /// The validated session, if one was found.
    pub session: Option<Session>,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for SessionExtractor
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Extract session from request extensions
        // The middleware should have already validated and inserted it
        let session = parts.extensions.get::<Session>().cloned();

        Ok(Self { session })
    }
}
// TAG: surface=security owner=platform-team rule=MID-001

/// Middleware function for session validation
pub async fn session_middleware<S: SessionStorage + 'static>(
    middleware: Arc<SessionMiddleware<S>>,
    mut request: Request,
    next: Next,
) -> Response {
    // Extract session ID from cookie
    let session_id = extract_session_cookie(&request, &middleware.config.cookie_name);

    if let Some(session_id) = session_id {
        match middleware.validate_session(&session_id).await {
            Ok(session) => {
                debug!("Valid session found for user {}", session.user_id);
                request.extensions_mut().insert(session);
            }
            Err(SessionError::Expired) => {
                warn!("Session {} expired", session_id);
                return (StatusCode::UNAUTHORIZED, "Session expired").into_response();
            }
            Err(SessionError::MaxAgeExceeded) => {
                warn!("Session {} max age exceeded", session_id);
                return (StatusCode::UNAUTHORIZED, "Session max age exceeded").into_response();
            }
            Err(SessionError::NotFound) => {
                warn!("Session {} not found", session_id);
                return (StatusCode::UNAUTHORIZED, "Session not found").into_response();
            }
            Err(err) => {
                warn!("Session validation error: {}", err);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Session validation failed",
                    // TAG: surface=security owner=security-team rule=SEC-001
                )
                    .into_response();
            }
        }
    } else {
        debug!("No session cookie found");
        return (StatusCode::UNAUTHORIZED, "No session").into_response();
    }

    next.run(request).await
}

/// Extract session ID from cookie
fn extract_session_cookie(request: &Request, cookie_name: &str) -> Option<String> {
    request
        .headers()
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|cookie| {
            let (name, value) = cookie.trim().split_once('=')?;

            if name == cookie_name {
                Some(value.to_string())
            } else {
                None
            }
        })
}

/// Create a Set-Cookie header value
// TAG: surface=security owner=platform-team rule=MID-001
#[must_use]
pub fn create_session_cookie(session_id: &str, config: &SessionConfig) -> String {
    let cookie_name = &config.cookie_name;
    let cookie_path = &config.cookie_path;
    let mut cookie = format!("{cookie_name}={session_id}");

    cookie.push_str("; Path=");
    cookie.push_str(cookie_path);

    if let Some(domain) = &config.cookie_domain {
        cookie.push_str("; Domain=");
        cookie.push_str(domain);
    }

    if config.cookie_secure {
        cookie.push_str("; Secure");
    }

    if config.cookie_http_only {
        cookie.push_str("; HttpOnly");
    }

    match config.cookie_same_site {
        SameSitePolicy::Strict => cookie.push_str("; SameSite=Strict"),
        SameSitePolicy::Lax => cookie.push_str("; SameSite=Lax"),
        SameSitePolicy::None => cookie.push_str("; SameSite=None"),
    }

    cookie
}

#[cfg(test)]
// TAG: surface=security owner=platform-team rule=GENERAL-001
mod tests;
