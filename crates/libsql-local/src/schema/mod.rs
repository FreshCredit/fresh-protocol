//! Database schema definitions for FreshCredit unified schema
//!
//! SCHEMA SOURCE OF TRUTH:
//! - Rust code: crates/db/libsql/local/src/lib.rs (initialize_schema function)
//! - SQL file: migrations/unified_schema.sql
//! - Cloud database: freshcredit-unified-schema-v1 (Turso)
//!
//! Module structure (REFACTORING IN PROGRESS):
//! - mod.rs (this file): Module exports
//! - core.rs: Core tables (user_profile, auth, preferences)
//! - financial.rs: Financial tables (accounts, transactions, balances)
//! - identity.rs: Identity tables (identity_verification, verified_credentials)
//! - blockchain.rs: Blockchain tables (blockchain_hashes, blockchain_proofs)
//! - staging.rs: Staging tables (staging_data, staging_status)
//! - healthkit.rs: HealthKit tables (healthkit_*)
//! - linkedin.rs: LinkedIn tables (linkedin_*)
//! - correlation.rs: Correlation tables (correlation_*)
//! - ai.rs: AI tables (ai_conversations, ai_messages)
//! - workflow.rs: Workflow tables (workflows, scoring_models)
//! - webhook.rs: Webhook tables (webhook_events)
//!
//! NOTE: Schema is currently defined in lib.rs initialize_schema() function.
//! This module will contain extracted schema definitions once refactoring is complete.
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

// Schema submodules will be added as tables are extracted from lib.rs
// pub mod core;
// pub mod financial;
// pub mod identity;
// pub mod blockchain;
// pub mod staging;
// pub mod healthkit;
// pub mod linkedin;
// pub mod correlation;
// pub mod ai;
// pub mod workflow;
// pub mod webhook;

