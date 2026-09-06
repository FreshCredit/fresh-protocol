// TAG: surface=security owner=security-team rule=SEC-001
//! `FreshCredit` Session Middleware
//!
//! This crate provides database-backed session management for the `FreshCredit` platform.
//! It replaces the previous client-side session management with a secure server-side
//! implementation using `LibSQL` for storage.
//!
//! ## Features
//!
//! - Database-backed session storage (`LibSQL`)
//! - Configurable timeout and max age
//! - Automatic session cleanup
//! - Axum middleware integration
//! - Secure cookie handling
//!
//! ## Usage
//!
//! ```rust,ignore
//! use freshcredit_middleware_session::{
//!     SessionMiddleware, SessionConfig, LibSqlSessionStorage,
//!     session_middleware,
//! };
//! use axum::{Router, routing::get};
//! use std::sync::Arc;
//!
//! async fn example() {
//!     // Create session storage
//!     let conn = Arc::new(/* LibSQL connection */);
//!     let storage = Arc::new(LibSqlSessionStorage::new(conn));
//!     storage.init().await.unwrap();
//!
//!     // Create session middleware
// TAG: surface=security owner=platform-team rule=MID-001
//!     let config = SessionConfig::default();
//!     let middleware = Arc::new(SessionMiddleware::new(storage, config));
//!
//!     // Add to Axum router
//!     let app = Router::new()
//!         .route("/protected", get(handler))
//!         .layer(axum::middleware::from_fn(move |req, next| {
//!             session_middleware(middleware.clone(), req, next)
//!         }));
//! }
//!
//! async fn handler() -> &'static str {
//!     "Protected route"
//! }
//! ```

#![forbid(unsafe_code)]

pub mod config;
pub mod libsql_storage;
pub mod middleware;
pub mod storage;
pub mod worker;

// Re-export commonly used types
pub use config::{SameSitePolicy, SessionConfig};
pub use libsql_storage::LibSqlSessionStorage;
pub use middleware::{
    create_session_cookie, session_middleware, SessionExtractor, SessionMiddleware,
};
pub use storage::{
    DeviceInfo, Session, SessionArtifact, SessionArtifactType, SessionError, SessionStorage,
};
pub use worker::SessionCleanupWorker;
