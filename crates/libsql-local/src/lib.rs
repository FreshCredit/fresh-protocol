//! Local `LibSQL` database operations for `FreshCredit`
//!
//! This module implements the unified database schema for `FreshCredit`,
//! // `HARDCODED_SCHEMA`: 117 tables in Rust modular schema, 118 in `unified_schema.sql` (+1 browser-specific `blockchain_proofs`) (verified 2026-01-15)
//! containing 117 tables that support:
// TAG: surface=database owner=platform-team rule=DB-001
//! - User profile and authentication (Entra ID + Verified ID)
//! - All 11 Plaid products (Accounts, Transactions, Auth, Identity, etc.)
//! - Payment processing (Stripe Connect ACH)
//! - AI features (conversations, file uploads)
//! - Workflow automation (Windmill integration)
//! - KILT Protocol DID support
//!
//! TABLE CONSOLIDATION NOTES:
//! - `reports` table is used for `BlockID` reports (NOT `credit_reports`)
//! - `identity_verification` (singular) is used for Plaid IDV (NOT `identity_verifications`)
//! - `balances` table stores balance history; `accounts` table columns store current balance
//!
//! SCHEMA SOURCE OF TRUTH:
//! - Rust code: `crates/db/libsql/local/src/schema/` modules (invoked by `initialize_schema` in this file)
//! - SQL file: `migrations/unified_schema.sql`
//! - Cloud database: freshcredit-unified-schema-v1 (Turso)
//!
//! Module structure:
//! - lib.rs (this file): Main entry point with `LocalClient`
//! - types.rs: All type definitions (`UserProfile`, `UserPreferences`, AI, Workflow, Webhook types)
//! - impls.rs: `LocalClient` struct and core implementation
//! - helpers.rs: Shared helper utilities
//! - schema/: Schema definitions organized by domain
//! - operations/: CRUD operations organized by domain
//!
//! `HARDCODED_SCHEMA`: 117 unique tables in modular schema (verified 2026-01-15)

// Submodules for incremental extraction
#![allow(clippy::wildcard_imports)]

// TAG: surface=database owner=platform-team rule=DB-001
/// Helper utilities for local `LibSQL` operations
pub mod helpers;
/// `LocalClient` struct and core implementation
pub mod impls;
/// Database CRUD operations organized by domain
pub mod operations;
/// Database schema definitions organized by domain
pub mod schema;
/// Type definitions for database records
pub mod types;

#[cfg(test)]
mod tests;

// Re-export types for backward compatibility
// All type definitions are in types.rs module
pub use types::{
    AiConversation, AiMessage, SaveUploadedFileParams, SchemaValidationResult, ScoringModelRecord,
    UploadedFile, UserPreferences, UserProfile, WebhookEvent, WebhookEventCounts, WorkflowRecord,
};

// Re-export the main client
pub use impls::LocalClient;
