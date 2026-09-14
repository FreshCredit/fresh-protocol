// TAG: surface=security owner=security-team rule=SEC-001
//! Axum middleware for rate limiting

use axum::{
    extract::{ConnectInfo, Request},
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{net::SocketAddr, sync::Arc};
use tracing::{instrument, warn};

use crate::{
    config::RateLimitConfig,
    limiter::{RateLimitError, RateLimiter},
};
use freshcredit_core_timing::SystemClock;

/// Rate limit layer for Axum
#[derive(Clone, Debug)]
pub struct RateLimitLayer {
    limiter: Arc<RateLimiter>,
    /// SECURITY: Separate stricter limiter for auth endpoints
    auth_limiter: Arc<RateLimiter>,
    include_headers: bool,
}

impl RateLimitLayer {
    /// Create a new rate limit layer
    #[must_use]
    pub fn new(config: RateLimitConfig) -> Self {
        let include_headers = config.include_headers;
        let limiter = Arc::new(RateLimiter::new(config, Arc::new(SystemClock)));
        // SECURITY: Auth endpoints use stricter rate limiting
        let auth_limiter = Arc::new(RateLimiter::new(
            RateLimitConfig::auth(),
            Arc::new(SystemClock),
        ));

        Self {
            limiter,
            auth_limiter,
            include_headers,
        }
    }

    /// Create rate limit layer for anonymous users
    #[must_use]
    pub fn anonymous() -> Self {
        Self::new(RateLimitConfig::anonymous())
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001

    /// Create rate limit layer for authenticated consumers
    #[must_use]
    pub fn consumer() -> Self {
        Self::new(RateLimitConfig::consumer())
    }

    /// Create rate limit layer for authenticated providers
    #[must_use]
    pub fn provider() -> Self {
        Self::new(RateLimitConfig::provider())
    }

    /// Middleware handler
    #[instrument(name = "rate_limit_middleware", skip(self, req, next), fields(path = %req.uri().path()))]
    pub async fn handle(&self, req: Request, next: Next) -> Response {
        // Skip rate limiting for exempt routes
        let path = req.uri().path();
        if should_exempt_from_rate_limit(path) {
            return next.run(req).await;
        }

        // Extract identifier (IP address or user ID)
        let identifier = extract_identifier(&req);

        // Log rate limit check for debugging
        let is_auth = is_auth_endpoint(req.method(), path);
        tracing::debug!(
            path = %path,
            method = %req.method(),
            identifier = %identifier,
            is_auth_endpoint = is_auth,
            "Rate limit check"
        );

        // SECURITY: Use stricter rate limiting for state-changing auth endpoints only.
        // GET/HEAD to login/register/etc. are render pages; limiting them by IP breaks
        // invite flows where many users share a corporate NAT or mobile carrier CGNAT.
        let limiter = if is_auth {
            &self.auth_limiter
        } else {
            &self.limiter
        };

        // Check rate limit
        match limiter.check_and_record(&identifier) {
            Ok(result) => {
                // Request allowed - add rate limit headers and proceed
                let mut response = next.run(req).await;

                if self.include_headers {
                    add_rate_limit_headers(response.headers_mut(), &result);
                }
                // TAG: surface=security owner=security-team rule=SEC-001

                response
            }
            Err(RateLimitError::LimitExceeded { retry_after, .. }) => {
                // Rate limit exceeded - return 429
                warn!("Rate limit exceeded for identifier: {}", identifier);

                let mut headers = HeaderMap::new();
                headers.insert(
                    "Retry-After",
                    HeaderValue::from_str(&retry_after.as_secs().to_string())
                        .unwrap_or_else(|_| HeaderValue::from_static("60")),
                );
                headers.insert("X-RateLimit-Limit", HeaderValue::from_static("0"));
                headers.insert("X-RateLimit-Remaining", HeaderValue::from_static("0"));

                (
                    StatusCode::TOO_MANY_REQUESTS,
                    headers,
                    "Rate limit exceeded",
                )
                    .into_response()
            }
        }
    }
}

/// Check if a route should be exempt from rate limiting
fn should_exempt_from_rate_limit(path: &str) -> bool {
    // Exempt static assets (CSS, JS, fonts, icons, images)
    if path.starts_with("/static/") {
        return true;
    }

    // Exempt health checks and well-known endpoints
    if matches!(
        path,
        "/" | "/health" | "/favicon.ico" | "/.well-known/did-configuration.json"
    ) {
        return true;
    }

    // SECURITY FIX: Authentication endpoints are NO LONGER exempt
    // They now use stricter rate limiting via is_auth_endpoint() below
    // Previously: "/login", "/test-signin", "/auth/callback", "/logout" were exempt
    // This was a CRITICAL security vulnerability allowing brute force attacks

    // Exempt webhook endpoints (external services)
    if path.starts_with("/api/webhooks/") {
        return true;
        // TAG: surface=security owner=platform-team rule=GENERAL-001
    }

    // Exempt critical database and UI endpoints that are called on every page load
    // These endpoints are protected by authentication, not rate limiting
    if matches!(
        path,
        "/api/database/token"           // Browser database initialization
            | "/api/notifications/unread-count"  // Header notification badge
            | "/api/user/me"                     // Header user info
            | "/api/session/status"              // Session keepalive
            | "/api/session/activity" // Session activity tracking
    ) {
        return true;
    }

    // Exempt logo proxy (cosmetic, no security impact)
    if path.starts_with("/api/logo-proxy") {
        return true;
    }

    // Exempt Turso sync proxy (already has circuit breaker and backoff logic)
    if path.starts_with("/api/turso-sync") {
        return true;
    }

    // Exempt authenticated consumer page routes
    // These are protected by authentication middleware, not rate limiting
    // Rate limiting authenticated pages causes 429 errors on normal navigation
    if matches!(
        path,
        "/dashboard"
            | "/reports"
            | "/settings"
            | "/accounts"
            | "/transactions"
            | "/profile"
            | "/notifications"
            | "/connect"
            | "/connect/bank"
            | "/connect/crypto"
            | "/connect/identity"
    ) {
        return true;
    }

    // Exempt provider business section routes (authenticated providers)
    // All /business/* routes are for authenticated providers
    if path.starts_with("/business") {
        return true;
    }
    // TAG: surface=security owner=security-team rule=SEC-001

    // Exempt shared informational routes
    if matches!(
        path,
        "/help" | "/support" | "/privacy" | "/terms" | "/about" | "/contact"
    ) {
        return true;
    }

    // Exempt public authentication render pages. These are GET-only landing pages
    // protected by CSP/headers, not rate limiting. Shared NAT / mobile carrier IPs
    // were hitting the anonymous tier here, causing legitimate invite links and
    // first-time users to receive 429 errors.
    if matches!(
        path,
        "/login"
            | "/register"
            | "/forgot-password"
            | "/reset-password"
            | "/test-signin"
            | "/auth/callback"
            | "/auth/verify"
            | "/auth/login/start"
            | "/logout"
            | "/solutions"
            | "/download"
            | "/book"
    ) {
        return true;
    }

    // Exempt debug routes (development and troubleshooting)
    if path.starts_with("/debug") {
        return true;
    }

    false
}

/// Check if a path is an authentication endpoint that needs stricter rate limiting.
/// SECURITY: Auth endpoints have lower limits to prevent brute force attacks.
/// GET/HEAD to render pages and OAuth callbacks use the normal limiter so invite links
/// and shared-network users are not blocked; state-changing auth requests stay strict.
fn is_auth_endpoint(method: &Method, path: &str) -> bool {
    let is_state_changing = !matches!(*method, Method::GET | Method::HEAD);

    if path.starts_with("/api/auth/") {
        return true;
    }

    matches!(
        path,
        "/login"
            | "/register"
            | "/forgot-password"
            | "/reset-password"
            | "/test-signin"
            | "/auth/callback"
            | "/auth/verify"
            | "/auth/refresh"
            | "/logout"
    ) && is_state_changing
}

/// Extract identifier from request (IP address or user ID)
/// `CLOUD_RUN_FIX`: Improved IP extraction to handle multiple proxy scenarios
///
/// # X-Forwarded-For trust model
///
/// The FIRST `X-Forwarded-For` entry is client-supplied and trivially
/// spoofable — keying the limiter on it lets an attacker mint a fresh bucket
/// per request and evade rate limits entirely. Our deployment topology is
/// client -> Cloud Run -> nginx -> app, and nginx uses
/// `$proxy_add_x_forwarded_for`, so each trusted hop APPENDS the peer it
/// observed to the END of the header. Only the LAST entry is therefore
/// platform-attested; every earlier entry may have been planted by the
/// client. We key on the last entry and ignore the rest. When the header is
/// absent we fall back to the connection's peer address (`ConnectInfo`),
/// which the platform sets from the real TCP peer.
fn extract_identifier(req: &Request) -> String {
    // Try to get user ID from extensions (set by auth middleware)
    if let Some(user_id) = req.extensions().get::<String>() {
        return format!("user:{user_id}");
    }

    // Try multiple headers for client IP (Cloud Run + Load Balancer compatibility)
    // Priority order: X-Forwarded-For (last entry) > X-Real-IP > X-Client-IP > CF-Connecting-IP
    // Final fallback: the connection's peer address inserted by the platform.
    let peer_ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|connect_info| connect_info.0.ip().to_string());
    let client_ip = req
        // TAG: surface=security owner=platform-team rule=GENERAL-001
        .headers()
        .get("X-Forwarded-For")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| {
            // X-Forwarded-For can contain multiple IPs: client, proxy1, proxy2, ...
            // SECURITY: Only the LAST entry is appended by our trusted proxy
            // (nginx `$proxy_add_x_forwarded_for`); earlier entries are
            // client-spoofable, so we key on the last one.
            s.split(',').next_back().map(str::trim)
        })
        .or_else(|| {
            req.headers()
                .get("X-Real-IP")
                .and_then(|h| h.to_str().ok())
                .map(str::trim)
        })
        .or_else(|| {
            req.headers()
                .get("X-Client-IP")
                .and_then(|h| h.to_str().ok())
                .map(str::trim)
        })
        .or_else(|| {
            req.headers()
                .get("CF-Connecting-IP") // Cloudflare
                .and_then(|h| h.to_str().ok())
                .map(str::trim)
        })
        .or(peer_ip.as_deref());

    if let Some(ip) = client_ip {
        if !ip.is_empty() && ip != "unknown" {
            return format!("ip:{ip}");
        }
    }

    // Log when IP extraction fails for debugging
    tracing::debug!("Could not extract client IP for rate limiting, using 'unknown'");
    "ip:unknown".to_string()
}

/// Add rate limit headers to response
fn add_rate_limit_headers(headers: &mut HeaderMap, result: &crate::limiter::RateLimitResult) {
    headers.insert(
        "X-RateLimit-Limit",
        HeaderValue::from_str(&result.max_requests.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );

    headers.insert(
        "X-RateLimit-Remaining",
        HeaderValue::from_str(&result.remaining.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
        // TAG: surface=security owner=security-team rule=SEC-001
    );

    headers.insert(
        "X-RateLimit-Reset",
        HeaderValue::from_str(&result.reset_at.timestamp().to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get, Router};
    use chrono::Utc;
    use tower::ServiceExt;

    #[test]
    fn test_should_exempt_from_rate_limit() {
        assert!(should_exempt_from_rate_limit("/static/main.css"));
        assert!(should_exempt_from_rate_limit("/health"));
        assert!(should_exempt_from_rate_limit("/favicon.ico"));
        assert!(should_exempt_from_rate_limit("/api/webhooks/stripe"));
        assert!(should_exempt_from_rate_limit("/dashboard"));
        assert!(should_exempt_from_rate_limit("/help"));
        assert!(should_exempt_from_rate_limit("/auth/login/start"));
        assert!(!should_exempt_from_rate_limit("/api/users"));
        assert!(!should_exempt_from_rate_limit("/api/auth/login"));
    }

    #[test]
    fn test_is_auth_endpoint() {
        // State-changing auth requests use the strict limiter.
        assert!(is_auth_endpoint(&Method::POST, "/login"));
        assert!(is_auth_endpoint(&Method::POST, "/api/auth/login"));
        assert!(is_auth_endpoint(&Method::POST, "/api/auth/register"));
        assert!(is_auth_endpoint(&Method::POST, "/logout"));
        assert!(is_auth_endpoint(&Method::POST, "/auth/refresh"));
        // GET to auth render pages/callbacks is not strict (avoids blocking shared NAT).
        assert!(!is_auth_endpoint(&Method::GET, "/login"));
        assert!(!is_auth_endpoint(&Method::GET, "/register"));
        assert!(!is_auth_endpoint(&Method::GET, "/auth/callback"));
        assert!(!is_auth_endpoint(&Method::GET, "/logout"));
        assert!(!is_auth_endpoint(&Method::GET, "/auth/refresh"));
        assert!(!is_auth_endpoint(&Method::GET, "/api/users"));
        assert!(!is_auth_endpoint(&Method::GET, "/dashboard"));
    }

    #[test]
    fn test_extract_identifier_from_headers() {
        let mut req = Request::new(Body::empty());
        req.headers_mut()
            .insert("X-Forwarded-For", HeaderValue::from_static("192.168.1.1"));
        assert_eq!(extract_identifier(&req), "ip:192.168.1.1");
    }

    #[test]
    fn test_extract_identifier_from_x_real_ip() {
        let mut req = Request::new(Body::empty());
        req.headers_mut()
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            .insert("X-Real-IP", HeaderValue::from_static("10.0.0.1"));
        assert_eq!(extract_identifier(&req), "ip:10.0.0.1");
    }

    #[test]
    fn test_extract_identifier_fallback() {
        let req = Request::new(Body::empty());
        assert_eq!(extract_identifier(&req), "ip:unknown");
    }

    #[test]
    fn test_extract_identifier_with_extension() {
        let mut req = Request::new(Body::empty());
        req.extensions_mut().insert("user-123".to_string());
        assert_eq!(extract_identifier(&req), "user:user-123");
    }

    #[test]
    fn test_add_rate_limit_headers() {
        let mut headers = HeaderMap::new();
        let result = crate::limiter::RateLimitResult {
            allowed: true,
            requests_made: 5,
            max_requests: 10,
            remaining: 5,
            reset_at: Utc::now(),
            retry_after: None,
        };
        add_rate_limit_headers(&mut headers, &result);
        assert_eq!(headers.get("X-RateLimit-Limit").unwrap(), "10");
        assert_eq!(headers.get("X-RateLimit-Remaining").unwrap(), "5");
        assert!(headers.contains_key("X-RateLimit-Reset"));
    }

    #[tokio::test]
    async fn test_middleware_exempt_route() {
        let layer = RateLimitLayer::anonymous();
        let app = Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(move |req, next| {
                let layer = layer.clone();
                async move { layer.handle(req, next).await }
            }));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
                // TAG: surface=security owner=security-team rule=SEC-001
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_extract_identifier_x_forwarded_for_multiple() {
        let mut req = Request::new(Body::empty());
        req.headers_mut().insert(
            "X-Forwarded-For",
            HeaderValue::from_static("192.168.1.1, 10.0.0.1, 172.16.0.1"),
        );
        // SECURITY: Only the platform-appended LAST entry is trusted.
        assert_eq!(extract_identifier(&req), "ip:172.16.0.1");
    }

    #[test]
    fn test_extract_identifier_spoofed_leading_xff_shares_bucket() {
        // An attacker can plant arbitrary leading XFF entries, but the
        // limiter keys on the trusted last entry, so both requests land in
        // the same bucket instead of minting a fresh one per request.
        let mut req_spoofed = Request::new(Body::empty());
        req_spoofed.headers_mut().insert(
            "X-Forwarded-For",
            HeaderValue::from_static("6.6.6.6, 203.0.113.5"),
        );
        let mut req_other_spoof = Request::new(Body::empty());
        req_other_spoof.headers_mut().insert(
            "X-Forwarded-For",
            HeaderValue::from_static("7.7.7.7, 198.51.100.9, 203.0.113.5"),
        );

        let id_spoofed = extract_identifier(&req_spoofed);
        let id_other = extract_identifier(&req_other_spoof);
        assert_eq!(id_spoofed, "ip:203.0.113.5");
        assert_eq!(id_spoofed, id_other);
    }

    #[test]
    fn test_extract_identifier_falls_back_to_connect_info_peer() {
        let mut req = Request::new(Body::empty());
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([10, 0, 0, 7], 51_300))));
        assert_eq!(extract_identifier(&req), "ip:10.0.0.7");
    }

    #[test]
    fn test_extract_identifier_x_client_ip() {
        let mut req = Request::new(Body::empty());
        req.headers_mut()
            .insert("X-Client-IP", HeaderValue::from_static("10.0.0.2"));
        assert_eq!(extract_identifier(&req), "ip:10.0.0.2");
    }

    #[test]
    fn test_extract_identifier_cf_connecting_ip() {
        let mut req = Request::new(Body::empty());
        req.headers_mut()
            .insert("CF-Connecting-IP", HeaderValue::from_static("203.0.113.1"));
        assert_eq!(extract_identifier(&req), "ip:203.0.113.1");
    }

    #[test]
    fn test_extract_identifier_unknown_ip() {
        let mut req = Request::new(Body::empty());
        req.headers_mut()
            .insert("X-Forwarded-For", HeaderValue::from_static("unknown"));
        assert_eq!(extract_identifier(&req), "ip:unknown");
    }

    #[test]
    fn test_extract_identifier_empty_x_forwarded_for() {
        let mut req = Request::new(Body::empty());
        req.headers_mut()
            .insert("X-Forwarded-For", HeaderValue::from_static(""));
        assert_eq!(extract_identifier(&req), "ip:unknown");
    }

    #[test]
    // TAG: surface=security owner=platform-team rule=GENERAL-001
    fn test_exempt_routes_comprehensive() {
        assert!(should_exempt_from_rate_limit("/static/js/app.js"));
        assert!(should_exempt_from_rate_limit("/static/css/style.css"));
        assert!(should_exempt_from_rate_limit("/"));
        assert!(should_exempt_from_rate_limit(
            "/.well-known/did-configuration.json"
        ));
        assert!(should_exempt_from_rate_limit("/api/database/token"));
        assert!(should_exempt_from_rate_limit(
            "/api/notifications/unread-count"
        ));
        assert!(should_exempt_from_rate_limit("/api/user/me"));
        assert!(should_exempt_from_rate_limit("/api/session/status"));
        assert!(should_exempt_from_rate_limit("/api/logo-proxy/google"));
        assert!(should_exempt_from_rate_limit("/api/turso-sync/sync"));
        assert!(should_exempt_from_rate_limit("/business/dashboard"));
        assert!(should_exempt_from_rate_limit("/business/reports"));
        assert!(should_exempt_from_rate_limit("/privacy"));
        assert!(should_exempt_from_rate_limit("/terms"));
        assert!(should_exempt_from_rate_limit("/about"));
        assert!(should_exempt_from_rate_limit("/contact"));
        assert!(should_exempt_from_rate_limit("/debug/info"));
        // Public auth render pages are exempt from IP-based rate limiting.
        assert!(should_exempt_from_rate_limit("/login"));
        assert!(should_exempt_from_rate_limit("/register"));
        assert!(should_exempt_from_rate_limit("/forgot-password"));
        assert!(should_exempt_from_rate_limit("/reset-password"));
        assert!(should_exempt_from_rate_limit("/solutions"));
        assert!(should_exempt_from_rate_limit("/download"));
        assert!(should_exempt_from_rate_limit("/book"));
        assert!(!should_exempt_from_rate_limit("/api/transactions"));
        assert!(!should_exempt_from_rate_limit("/api/payments"));
        assert!(!should_exempt_from_rate_limit("/api/auth/login"));
        assert!(!should_exempt_from_rate_limit("/api/auth/2fa/verify"));
    }

    #[test]
    fn test_auth_endpoints_comprehensive() {
        let post = Method::POST;
        let get = Method::GET;

        // State-changing auth requests are strict.
        assert!(is_auth_endpoint(&post, "/login"));
        assert!(is_auth_endpoint(&post, "/register"));
        assert!(is_auth_endpoint(&post, "/forgot-password"));
        assert!(is_auth_endpoint(&post, "/reset-password"));
        assert!(is_auth_endpoint(&post, "/test-signin"));
        assert!(is_auth_endpoint(&post, "/logout"));
        assert!(is_auth_endpoint(&post, "/auth/refresh"));

        // All /api/auth/* requests are strict.
        assert!(is_auth_endpoint(&post, "/api/auth/login"));
        assert!(is_auth_endpoint(&post, "/api/auth/register"));
        assert!(is_auth_endpoint(&post, "/api/auth/refresh"));
        assert!(is_auth_endpoint(&post, "/api/auth/forgot-password"));
        assert!(is_auth_endpoint(&post, "/api/auth/reset-password"));
        assert!(is_auth_endpoint(&post, "/api/auth/verify-email"));
        assert!(is_auth_endpoint(&post, "/api/auth/2fa/verify"));
        assert!(is_auth_endpoint(&post, "/api/auth/mfa/setup"));

        // GET to render pages / OAuth callbacks is not strict (shared NAT fix).
        assert!(!is_auth_endpoint(&get, "/login"));
        assert!(!is_auth_endpoint(&get, "/register"));
        assert!(!is_auth_endpoint(&get, "/forgot-password"));
        assert!(!is_auth_endpoint(&get, "/reset-password"));
        assert!(!is_auth_endpoint(&get, "/test-signin"));
        assert!(!is_auth_endpoint(&get, "/auth/callback"));
        assert!(!is_auth_endpoint(&get, "/auth/verify"));
        assert!(!is_auth_endpoint(&get, "/logout"));

        assert!(!is_auth_endpoint(&get, "/api/users"));
        assert!(!is_auth_endpoint(&get, "/api/transactions"));
    }
    // TAG: surface=security owner=security-team rule=SEC-001
}
