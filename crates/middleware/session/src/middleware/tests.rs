// TAG: surface=security owner=security-team rule=SEC-001
use super::*;
use crate::SessionArtifact;
use async_trait::async_trait;
use axum::body::Body;
use axum::http::Request;
use axum::{routing::get, Router};
use chrono::{DateTime, Utc};
use std::sync::Mutex;
use tower::util::ServiceExt;

struct MockStorage {
    session: Mutex<Option<Session>>,
    get_and_refresh_result: Mutex<Option<Result<Option<Session>, SessionError>>>,
    delete_called: Mutex<bool>,
    create_called: Mutex<bool>,
}

impl MockStorage {
    fn with_session(session: Session) -> Self {
        Self {
            session: Mutex::new(Some(session)),
            get_and_refresh_result: Mutex::new(None),
            delete_called: Mutex::new(false),
            create_called: Mutex::new(false),
        }
    }

    fn with_get_and_refresh_result(result: Result<Option<Session>, SessionError>) -> Self {
        Self {
            session: Mutex::new(None),
            get_and_refresh_result: Mutex::new(Some(result)),
            delete_called: Mutex::new(false),
            create_called: Mutex::new(false),
        }
    }
}

#[async_trait]
impl SessionStorage for MockStorage {
    async fn create(&self, _session: Session) -> Result<(), SessionError> {
        *self.create_called.lock().unwrap() = true;
        Ok(())
    }

    async fn get(&self, _session_id: &str) -> Result<Option<Session>, SessionError> {
        // TAG: surface=security owner=platform-team rule=MID-001
        Ok(self.session.lock().unwrap().clone())
    }

    async fn get_and_refresh(
        &self,
        _session_id: &str,
        _timeout: chrono::Duration,
    ) -> Result<Option<Session>, SessionError> {
        let result = self.get_and_refresh_result.lock().unwrap().clone();
        if let Some(result) = result {
            return result;
        }
        Ok(self.session.lock().unwrap().clone())
    }

    async fn update_activity(
        &self,
        _session_id: &str,
        _timeout: chrono::Duration,
    ) -> Result<(), SessionError> {
        Ok(())
    }

    async fn delete(&self, _session_id: &str) -> Result<(), SessionError> {
        *self.delete_called.lock().unwrap() = true;
        Ok(())
    }

    async fn cleanup_expired(&self) -> Result<u64, SessionError> {
        Ok(0)
    }

    async fn get_user_sessions(&self, _user_id: &str) -> Result<Vec<Session>, SessionError> {
        Ok(vec![])
    }

    async fn record_artifact(&self, _artifact: SessionArtifact) -> Result<(), SessionError> {
        Ok(())
    }

    async fn get_session_artifacts(
        &self,
        _session_id: &str,
    ) -> Result<Vec<SessionArtifact>, SessionError> {
        Ok(vec![])
    }

    // TAG: surface=security owner=platform-team rule=MID-001
    async fn get_user_artifacts(
        &self,
        _user_id: &str,
        _limit: i64,
    ) -> Result<Vec<SessionArtifact>, SessionError> {
        Ok(vec![])
    }

    async fn store_refresh_token(
        &self,
        _session_id: &str,
        _refresh_token: &str,
        _expires_at: DateTime<Utc>,
    ) -> Result<(), SessionError> {
        Ok(())
    }

    async fn get_refresh_token(&self, _session_id: &str) -> Result<Option<String>, SessionError> {
        Ok(None)
    }

    async fn update_tokens(
        &self,
        _session_id: &str,
        _access_token: &str,
        _refresh_token: Option<&str>,
        _expires_in: i64,
    ) -> Result<(), SessionError> {
        Ok(())
    }

    async fn get_by_access_token(
        &self,
        _access_token: &str,
    ) -> Result<Option<Session>, SessionError> {
        Ok(None)
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_refresh_event(
        &self,
        _session_id: &str,
        _user_id: &str,
        _event_type: &str,
        _ip_address: Option<&str>,
        _user_agent: Option<&str>,
        _success: bool,
        _error_message: Option<&str>,
        // TAG: surface=security owner=platform-team rule=MID-001
    ) -> Result<(), SessionError> {
        Ok(())
    }

    async fn check_refresh_rate_limit(
        &self,
        _session_id: &str,
        _ip_address: &str,
    ) -> Result<bool, SessionError> {
        Ok(true)
    }

    async fn increment_refresh_rate_limit(
        &self,
        _session_id: &str,
        _ip_address: &str,
    ) -> Result<(), SessionError> {
        Ok(())
    }
}

fn test_config() -> SessionConfig {
    SessionConfig {
        timeout: chrono::Duration::minutes(30),
        max_age: chrono::Duration::hours(24),
        cookie_name: "test_session".to_string(),
        cookie_domain: None,
        cookie_path: "/".to_string(),
        cookie_secure: true,
        cookie_http_only: true,
        cookie_same_site: SameSitePolicy::Lax,
        cleanup_interval: chrono::Duration::hours(1),
    }
}

fn valid_session() -> Session {
    Session::new(
        "user_1".to_string(),
        chrono::Duration::minutes(30),
        chrono::Duration::hours(24),
    )
}

#[tokio::test]
async fn test_validate_session_valid() {
    let session = valid_session();
    let storage = Arc::new(MockStorage::with_session(session.clone()));
    // TAG: surface=security owner=platform-team rule=MID-001
    let middleware = SessionMiddleware::new(storage, test_config());

    let result = middleware.validate_session(&session.id).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().id, session.id);
}

#[tokio::test]
async fn test_validate_session_not_found() {
    let storage = Arc::new(MockStorage::with_get_and_refresh_result(Ok(None)));
    let middleware = SessionMiddleware::new(storage, test_config());

    let result = middleware.validate_session("nonexistent").await;
    assert!(matches!(result, Err(SessionError::NotFound)));
}

#[tokio::test]
async fn test_validate_session_expired() {
    let mut session = valid_session();
    session.expires_at = Utc::now() - chrono::Duration::minutes(1);
    let storage = Arc::new(MockStorage::with_session(session));
    let middleware = SessionMiddleware::new(storage.clone(), test_config());

    let result = middleware.validate_session("test_id").await;
    assert!(matches!(result, Err(SessionError::Expired)));
    assert!(*storage.delete_called.lock().unwrap());
}

#[tokio::test]
async fn test_validate_session_max_age_exceeded() {
    let mut session = valid_session();
    session.created_at = Utc::now() - chrono::Duration::hours(25);
    let storage = Arc::new(MockStorage::with_session(session));
    let middleware = SessionMiddleware::new(storage.clone(), test_config());

    let result = middleware.validate_session("test_id").await;
    assert!(matches!(result, Err(SessionError::MaxAgeExceeded)));
    assert!(*storage.delete_called.lock().unwrap());
}

#[tokio::test]
async fn test_validate_session_storage_error() {
    let storage = Arc::new(MockStorage::with_get_and_refresh_result(Err(
        SessionError::DatabaseError("db fail".to_string()),
    )));
    let middleware = SessionMiddleware::new(storage, test_config());

    let result = middleware.validate_session("test_id").await;
    // TAG: surface=security owner=platform-team rule=MID-001
    assert!(matches!(result, Err(SessionError::DatabaseError(_))));
}

#[tokio::test]
async fn test_create_session() {
    let storage = Arc::new(MockStorage::with_get_and_refresh_result(Ok(None)));
    let middleware = SessionMiddleware::new(storage.clone(), test_config());

    let result = middleware.create_session("user_1".to_string()).await;
    assert!(result.is_ok());
    assert!(*storage.create_called.lock().unwrap());
}

#[tokio::test]
async fn test_delete_session() {
    let storage = Arc::new(MockStorage::with_get_and_refresh_result(Ok(None)));
    let middleware = SessionMiddleware::new(storage.clone(), test_config());

    middleware.delete_session("test_id").await.unwrap();
    assert!(*storage.delete_called.lock().unwrap());
}

#[tokio::test]
async fn test_session_middleware_no_cookie() {
    let storage = Arc::new(MockStorage::with_get_and_refresh_result(Ok(None)));
    let middleware = Arc::new(SessionMiddleware::new(storage, test_config()));

    let app =
        Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(move |req, next| {
                let mw = middleware.clone();
                async move { session_middleware(mw, req, next).await }
            }));

    let response = app
        .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_session_middleware_valid_cookie() {
    let session = valid_session();
    let storage = Arc::new(MockStorage::with_session(session));
    // TAG: surface=security owner=platform-team rule=MID-001
    let middleware = Arc::new(SessionMiddleware::new(storage, test_config()));

    let app =
        Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(move |req, next| {
                let mw = middleware.clone();
                async move { session_middleware(mw, req, next).await }
            }));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/test")
                .header(header::COOKIE, "test_session=session_123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_session_middleware_expired_cookie() {
    let mut session = valid_session();
    session.expires_at = Utc::now() - chrono::Duration::minutes(1);
    let storage = Arc::new(MockStorage::with_session(session));
    let middleware = Arc::new(SessionMiddleware::new(storage, test_config()));

    let app =
        Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(move |req, next| {
                let mw = middleware.clone();
                async move { session_middleware(mw, req, next).await }
            }));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/test")
                .header(header::COOKIE, "test_session=session_123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        // TAG: surface=security owner=platform-team rule=MID-001
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_session_middleware_not_found() {
    let storage = Arc::new(MockStorage::with_get_and_refresh_result(Ok(None)));
    let middleware = Arc::new(SessionMiddleware::new(storage, test_config()));

    let app =
        Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(move |req, next| {
                let mw = middleware.clone();
                async move { session_middleware(mw, req, next).await }
            }));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/test")
                .header(header::COOKIE, "test_session=session_123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_session_middleware_storage_error() {
    let storage = Arc::new(MockStorage::with_get_and_refresh_result(Err(
        SessionError::DatabaseError("db fail".to_string()),
    )));
    let middleware = Arc::new(SessionMiddleware::new(storage, test_config()));

    let app =
        Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(move |req, next| {
                let mw = middleware.clone();
                async move { session_middleware(mw, req, next).await }
            }));

    // TAG: surface=security owner=platform-team rule=MID-001
    let response = app
        .oneshot(
            Request::builder()
                .uri("/test")
                .header(header::COOKIE, "test_session=session_123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_session_middleware_wrong_cookie_name() {
    let session = valid_session();
    let storage = Arc::new(MockStorage::with_session(session));
    let middleware = Arc::new(SessionMiddleware::new(storage, test_config()));

    let app =
        Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(move |req, next| {
                let mw = middleware.clone();
                async move { session_middleware(mw, req, next).await }
            }));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/test")
                .header(header::COOKIE, "wrong_name=session_123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_session_middleware_max_age_exceeded() {
    let mut session = valid_session();
    session.created_at = Utc::now() - chrono::Duration::hours(25);
    let storage = Arc::new(MockStorage::with_session(session));
    let middleware = Arc::new(SessionMiddleware::new(storage, test_config()));
    // TAG: surface=security owner=platform-team rule=MID-001

    let app =
        Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(move |req, next| {
                let mw = middleware.clone();
                async move { session_middleware(mw, req, next).await }
            }));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/test")
                .header(header::COOKIE, "test_session=session_123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn test_create_session_cookie_basic() {
    let config = test_config();
    let cookie = create_session_cookie("sid_123", &config);
    assert!(cookie.contains("test_session=sid_123"));
    assert!(cookie.contains("Path=/"));
    assert!(cookie.contains("Secure"));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Lax"));
}

#[test]
fn test_create_session_cookie_with_domain() {
    let mut config = test_config();
    config.cookie_domain = Some("example.com".to_string());
    let cookie = create_session_cookie("sid_123", &config);
    assert!(cookie.contains("Domain=example.com"));
}

#[test]
fn test_create_session_cookie_same_site_strict() {
    let mut config = test_config();
    config.cookie_same_site = SameSitePolicy::Strict;
    let cookie = create_session_cookie("sid_123", &config);
    // TAG: surface=security owner=platform-team rule=MID-001
    assert!(cookie.contains("SameSite=Strict"));
}

#[test]
fn test_create_session_cookie_same_site_none() {
    let mut config = test_config();
    config.cookie_same_site = SameSitePolicy::None;
    let cookie = create_session_cookie("sid_123", &config);
    assert!(cookie.contains("SameSite=None"));
}

#[test]
fn test_create_session_cookie_not_secure() {
    let mut config = test_config();
    config.cookie_secure = false;
    let cookie = create_session_cookie("sid_123", &config);
    assert!(!cookie.contains("Secure"));
}

#[test]
fn test_create_session_cookie_not_http_only() {
    let mut config = test_config();
    config.cookie_http_only = false;
    let cookie = create_session_cookie("sid_123", &config);
    assert!(!cookie.contains("HttpOnly"));
}

#[tokio::test]
async fn test_session_extractor() {
    let session = valid_session();
    let (mut parts, _body) = Request::new(Body::empty()).into_parts();
    parts.extensions.insert(session.clone());

    let result = SessionExtractor::from_request_parts(&mut parts, &&()).await;
    assert!(result.is_ok());
    let extractor = result.unwrap();
    assert!(extractor.session.is_some());
    assert_eq!(extractor.session.unwrap().id, session.id);
}

#[tokio::test]
async fn test_session_extractor_no_session() {
    let (mut parts, _body) = Request::new(Body::empty()).into_parts();

    let result = SessionExtractor::from_request_parts(&mut parts, &&()).await;
    assert!(result.is_ok());
    let extractor = result.unwrap();
    assert!(extractor.session.is_none());
}
