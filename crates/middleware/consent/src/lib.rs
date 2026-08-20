// TAG: surface=security owner=security-team rule=SEC-001
//! `FreshCredit` Consent Middleware
//!
//! This crate provides consent management and enforcement for the `FreshCredit` platform.
//! It implements jurisdiction-specific consent rules (GDPR, CCPA, FCRA) and ensures
//! that data access only occurs with valid, non-expired consent.
//!
//! ## Features
//!
//! - Jurisdiction-specific consent rules (GDPR, CCPA, FCRA)
//! - Database-backed consent storage (`LibSQL`)
//! - Automatic consent expiration enforcement
//! - Consent validation middleware for Axum
//! - Background cleanup worker for expired consents
//!
//! ## Consent Types
//!
//! - Financial data access (Plaid) - 90 days (US), 12 months (GDPR)
//! - `LinkedIn` data access - 30 days (US), 12 months (GDPR)
//! - Credit report generation (FCRA) - 2 years
//! - Report sharing - 1 year
//! - Payment processing - No expiration (per transaction)
//! - Marketing communications - 1 year
//! - Analytics - 1 year (GDPR), no expiration (CCPA)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use freshcredit_middleware_consent::{
//!     ConsentMiddleware, ConsentRequirement, LibSqlConsentStorage,
//!     ConsentType, Jurisdiction, consent_middleware,
//! };
//! use axum::{Router, routing::get};
//! use std::sync::Arc;
//!
//! async fn example() {
//!     // Create consent storage
//!     let conn = Arc::new(/* LibSQL connection */);
//!     let storage = Arc::new(LibSqlConsentStorage::new(conn));
//!
//!     // Create consent middleware
//!     let middleware = Arc::new(ConsentMiddleware::new(
//!         storage,
// TAG: surface=security owner=platform-team rule=MID-001
//!         Jurisdiction::US,
//!     ));
//!
//!     // Define consent requirement
//!     let requirement = ConsentRequirement::single(
//!         ConsentType::FinancialDataAccess
//!     );
//!
//!     // Add to Axum router
//!     let app = Router::new()
//!         .route("/plaid/data", get(handler))
//!         .layer(axum::middleware::from_fn(move |req, next| {
//!             consent_middleware(
//!                 middleware.clone(),
//!                 requirement.clone(),
//!                 req,
//!                 next,
//!             )
//!         }));
//! }
//!
//! async fn handler() -> &'static str {
//!     "Protected route requiring consent"
//! }
//! ```

pub mod libsql_storage;
pub mod middleware;
pub mod rules;
pub mod storage;
pub mod types;
pub mod validator;
pub mod worker;

// Re-export commonly used types
pub use libsql_storage::LibSqlConsentStorage;
pub use middleware::{consent_middleware, ConsentExtractor, ConsentMiddleware, ConsentRequirement};
pub use rules::ConsentRules;
pub use storage::{ConsentQuery, ConsentStorage, CreateConsentRequest};
pub use types::{Consent, ConsentError, ConsentType, ConsentValidation, Jurisdiction};
pub use validator::ConsentValidator;
pub use worker::ConsentCleanupWorker;
