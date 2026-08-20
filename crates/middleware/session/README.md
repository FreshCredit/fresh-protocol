---
title: "freshcredit-middleware-session"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "middleware"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# freshcredit-middleware-session

Database-backed session management for FreshCredit.

## Purpose

Provides secure server-side session management using LibSQL for storage. Replaces client-side session management with configurable timeout, automatic cleanup, and secure cookie handling. Integrates as Axum middleware.

## Directory Structure

```
crates/middleware/session/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    ├── lib.rs          # Module exports
    ├── config.rs       # Session configuration
    ├── libsql_storage.rs  # LibSQL storage backend
    ├── middleware.rs   # Axum middleware
    ├── storage.rs      # Storage trait
    └── worker.rs       # Cleanup worker
```

## Key Types

| Name | Type | Description |
|------|------|-------------|
| `SessionMiddleware` | struct | Axum middleware |
| `SessionConfig` | struct | Configuration |
| `LibSqlSessionStorage` | struct | LibSQL storage |
| `Session` | struct | Session data |
| `SessionStorage` | trait | Storage abstraction |
| `SessionCleanupWorker` | struct | Background cleanup |
| `SameSitePolicy` | enum | Cookie SameSite policy |

## Integration

```rust
use freshcredit_middleware_session::{
    SessionMiddleware, SessionConfig, LibSqlSessionStorage,
    session_middleware,
};
use axum::{Router, routing::get};
use std::sync::Arc;

async fn example() {
    // Create session storage
    let conn = Arc::new(/* LibSQL connection */);
    let storage = Arc::new(LibSqlSessionStorage::new(conn));
    storage.init().await.unwrap();
    
    // Create session middleware
    let config = SessionConfig::default();
    let middleware = Arc::new(SessionMiddleware::new(storage, config));
    
    // Add to Axum router
    let app = Router::new()
        .route("/protected", get(handler))
        .layer(axum::middleware::from_fn(move |req, next| {
            session_middleware(middleware.clone(), req, next)
        }));
}

async fn handler() -> &'static str {
    "Protected route"
}
```

## Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `timeout_seconds` | 3600 | Session timeout (1 hour) |
| `max_age_seconds` | 86400 | Cookie max age (24 hours) |
| `secure` | true | Secure cookie flag |
| `http_only` | true | HttpOnly cookie flag |
| `same_site` | Lax | SameSite policy |
| `cookie_name` | "session" | Cookie name |

## Session Storage

```rust
use freshcredit_middleware_session::{
    SessionStorage, Session, DeviceInfo
};

#[async_trait]
impl SessionStorage for MyStorage {
    async fn create_session(&self, session: &Session) -> Result<String>;
    async fn get_session(&self, id: &str) -> Result<Option<Session>>;
    async fn update_session(&self, id: &str, session: &Session) -> Result<()>;
    async fn delete_session(&self, id: &str) -> Result<()>;
    async fn cleanup_expired(&self) -> Result<u64>;
}
```

## Cleanup Worker

```rust
use freshcredit_middleware_session::SessionCleanupWorker;

let worker = SessionCleanupWorker::new(storage, Duration::from_secs(3600));
worker.start().await?;
```

## Middleware Behavior

```
Request → SessionMiddleware → Extract Cookie → Lookup Session → Handler
                                    ↓              ↓
                                No Cookie?    Expired?
                                    ↓              ↓
                                Create New    Return 401
```

## Dependencies

### External
- `axum` - Web framework
- `tower` / `tower-http` - Middleware
- `libsql` - Database storage
- `tokio` - Async runtime
- `async-trait` - Async traits
- `serde` / `serde_json` - Serialization
- `chrono` - Date/time
- `thiserror` / `anyhow` - Error handling
- `tracing` - Logging
- `uuid` - UUID generation

### Internal
- `freshcredit-core-timing` - Clock abstraction

## Testing

```bash
cargo test -p freshcredit-middleware-session
```

## Related Documentation

📁 [freshcredit-core-timing](../../core/timing/)

📁 [freshcredit-middleware-consent](../consent/)
