// TAG: surface=security owner=security-team rule=SEC-001
//! Rate limiting middleware for `FreshCredit` API endpoints
//!
//! Provides two types of rate limiting:
//!
//! 1. **HTTP Middleware Rate Limiting** (sliding window)
//!    - Applied globally via `RateLimitLayer`
//!    - Tier-based: Anonymous (200/min), Consumer (300/min), Provider (600/min)
//!
//! 2. **Operation-Specific Rate Limiting** (token bucket)
//!    - Applied per-operation via `OperationRateLimiterService`
//!    - Fine-grained limits for expensive operations like Plaid sync, report generation
//!
//! ## Usage
//!
//! ```rust,ignore
//! // HTTP middleware (in app setup)
//! let rate_limit_layer = RateLimitLayer::new(RateLimitConfig::anonymous(), clock.clone());
//!
//! // Operation-specific (injected as singleton)
//! let op_limiter = Arc::new(OperationRateLimiterService::new(clock));
//! // In route handler:
//! op_limiter.check_operation(user_id, RateLimitedOperation::ReportGeneration, tier)?;
//! ```

#![allow(clippy::wildcard_imports)]
#![forbid(unsafe_code)]

pub mod config;
pub mod limiter;
pub mod middleware;
/// Operation-specific rate limiting (token bucket).
pub mod operation;

pub use config::{RateLimitConfig, RateLimitTier};
pub use limiter::{RateLimitError, RateLimitResult, RateLimiter};
pub use middleware::RateLimitLayer;
pub use operation::{
    OperationRateLimitConfig, OperationRateLimitError, OperationRateLimitResult,
    OperationRateLimiterService, RateLimitedOperation,
};
