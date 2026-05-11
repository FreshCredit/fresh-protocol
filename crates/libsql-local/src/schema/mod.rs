//! Database schema definitions for `FreshCredit` unified schema
//!
//! SCHEMA SOURCE OF TRUTH:
//! - Rust code: crates/db/libsql/local/src/lib.rs (`initialize_schema` function)
//! - SQL file: `migrations/unified_schema.sql`
//! - Cloud database: freshcredit-unified-schema-v1 (Turso)
//!
//! Module structure:
//! - mod.rs (this file): Module exports, re-exports
//! - category.rs: `SchemaCategory` enum
//! - types.rs: Schema type definitions
//! - impls.rs: Schema initialization implementations
//! - helpers.rs: Helper functions
//! - core.rs: Core tables (`user_profile`, `auth_tokens`, `user_preferences`)
//! - financial.rs: Financial tables (accounts, transactions, balances)
//! - identity.rs: Identity tables (`identity_verification`, `verified_credentials`)
//! - ai.rs: AI tables (`ai_conversations`, `ai_messages`, `uploaded_files`)
//! - workflow.rs: Workflow tables (workflows)
//! - webhook.rs: Webhook tables (`webhook_events`, notifications)
//! - plaid.rs: Plaid product tables (assets, balances, `consumer_reports`, etc.)
//! - payments.rs: Payment tables (customers, `funding_sources`, payments, etc.)
//! - reports.rs: Reports and scoring tables (reports, scores, offers, etc.)
//! - ticketing.rs: Ticketing system tables (tickets, `ticket_comments`, etc.)
//! - compliance.rs: Compliance monitoring tables (`compliance_scans`, etc.)
//! - notifications.rs: Notification tables (`webhook_events`, notifications)
//! - linkedin.rs: `LinkedIn` professional data tables
//! - healthkit.rs: Apple `HealthKit` tables
//! - `apple_music.rs`: Apple Music tables
//! - correlation.rs: Correlation engine tables
//! - platform.rs: Platform-level tables (referrals, metrics, etc.)
//! - indexes.rs: All database indexes
//!
//! REFACTORING STATUS:
//! - ✅ core.rs: Extracted (`user_profile`, `auth_tokens`, `user_preferences`)
//! - ✅ financial.rs: Extracted (accounts, transactions, balances)
//! - ✅ identity.rs: Extracted (`identity_verification`, `verified_credentials`)
//! - ✅ ai.rs: Extracted (`ai_conversations`, `ai_messages`, `uploaded_files`)
//! - ✅ workflow.rs: Extracted (workflows)
//! - ✅ webhook.rs: Extracted (`webhook_events`, notifications)
//! - ✅ plaid.rs: Extracted (Plaid product tables)
//! - ✅ payments.rs: Extracted (payment tables)
//! - ✅ reports.rs: Extracted (reports, scores, offers)
//! - ✅ ticketing.rs: Extracted (ticketing system)
//! - ✅ compliance.rs: Extracted (compliance monitoring)
//! - ✅ notifications.rs: Extracted (notifications)
//! - ✅ linkedin.rs: Extracted (`LinkedIn` tables)
//! - ✅ healthkit.rs: Extracted (`HealthKit` tables)
//! - ✅ `apple_music.rs`: Extracted (Apple Music tables)
//! - ✅ correlation.rs: Extracted (correlation tables)
//! - ✅ platform.rs: Extracted (platform tables)
//! - ✅ indexes.rs: Extracted (all indexes)
//!
//! NOTE: `blockchain_hash` is a column in multiple tables, not a separate table group.
//! NOTE: staging tables are in the staging crate, not local schema.
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

// Core schema modules
pub mod ai;
pub mod core;
pub mod financial;
pub mod identity;
pub mod webhook;
pub mod workflow;

// Plaid and payment modules
pub mod payments;
pub mod plaid;

// Business logic modules
pub mod compliance;
pub mod customers;
pub mod notifications;
pub mod reports;
pub mod teams;
pub mod ticketing;

// Data source modules
pub mod apple_music;
pub mod correlation;
pub mod healthkit;
pub mod ip;
pub mod linkedin;
pub mod publications;

// Platform and infrastructure modules
pub mod indexes;
pub mod platform;
pub mod security;

// Commerce modules (UCP)
pub mod ucp;

// Agent modules
pub mod agent;

// Governance modules
pub mod governance;

// Re-exports for convenience
pub use agent::{
    check_agent_bindings_schema,
    initialize_agent_tables,
};
pub use ai::initialize_ai_tables;
pub use apple_music::initialize_apple_music_tables;
pub use compliance::initialize_compliance_tables;
pub use core::{
    initialize_core_indexes,
    initialize_core_tables,
};
pub use correlation::initialize_correlation_tables;
pub use customers::{
    initialize_customer_indexes,
    initialize_customer_tables,
};
pub use financial::initialize_financial_tables;
pub use governance::{
    initialize_governance_indexes,
    initialize_governance_tables,
};
pub use healthkit::initialize_healthkit_tables;
pub use identity::initialize_identity_tables;
pub use indexes::initialize_all_indexes;
pub use ip::initialize_ip_tables;
pub use linkedin::initialize_linkedin_tables;
pub use notifications::initialize_notification_tables;
pub use payments::initialize_payment_tables;
pub use plaid::initialize_all_plaid_tables;
pub use platform::initialize_platform_tables;
pub use publications::initialize_publication_tables;
pub use reports::initialize_all_reports_tables;
pub use security::initialize_security_tables;
pub use teams::{
    initialize_teams_indexes,
    initialize_teams_tables,
};
pub use ticketing::initialize_ticketing_tables;
pub use ucp::initialize_ucp_tables;
pub use webhook::initialize_webhook_tables;
pub use workflow::initialize_workflow_tables;

pub mod category;
pub use category::SchemaCategory;

mod types;
pub use types::SchemaValidation;

mod helpers;
pub use helpers::try_create_index;

mod impls;
pub use impls::{
    initialize_all_schema_tables,
    initialize_extracted_tables,
};

#[cfg(test)]
mod tests;
