//! Database schema definitions for FreshCredit unified schema
//!
//! SCHEMA SOURCE OF TRUTH:
//! - Rust code: crates/db/libsql/local/src/lib.rs (initialize_schema function)
//! - SQL file: migrations/unified_schema.sql
//! - Cloud database: freshcredit-unified-schema-v1 (Turso)
//!
//! Module structure:
//! - mod.rs (this file): Module exports, SchemaCategory enum, unified initialization
//! - core.rs: Core tables (user_profile, auth_tokens, user_preferences)
//! - financial.rs: Financial tables (accounts, transactions, balances)
//! - identity.rs: Identity tables (identity_verification, verified_credentials)
//! - ai.rs: AI tables (ai_conversations, ai_messages, uploaded_files)
//! - workflow.rs: Workflow tables (workflows)
//! - webhook.rs: Webhook tables (webhook_events, notifications)
//! - plaid.rs: Plaid product tables (assets, balances, consumer_reports, etc.)
//! - payments.rs: Payment tables (customers, funding_sources, payments, etc.)
//! - reports.rs: Reports and scoring tables (reports, scores, offers, etc.)
//! - ticketing.rs: Ticketing system tables (tickets, ticket_comments, etc.)
//! - compliance.rs: Compliance monitoring tables (compliance_scans, etc.)
//! - notifications.rs: Notification tables (webhook_events, notifications)
//! - linkedin.rs: LinkedIn professional data tables
//! - healthkit.rs: Apple HealthKit tables
//! - apple_music.rs: Apple Music tables
//! - correlation.rs: Correlation engine tables
//! - platform.rs: Platform-level tables (referrals, metrics, etc.)
//! - indexes.rs: All database indexes
//!
//! REFACTORING STATUS:
//! - ✅ core.rs: Extracted (user_profile, auth_tokens, user_preferences)
//! - ✅ financial.rs: Extracted (accounts, transactions, balances)
//! - ✅ identity.rs: Extracted (identity_verification, verified_credentials)
//! - ✅ ai.rs: Extracted (ai_conversations, ai_messages, uploaded_files)
//! - ✅ workflow.rs: Extracted (workflows)
//! - ✅ webhook.rs: Extracted (webhook_events, notifications)
//! - ✅ plaid.rs: Extracted (Plaid product tables)
//! - ✅ payments.rs: Extracted (payment tables)
//! - ✅ reports.rs: Extracted (reports, scores, offers)
//! - ✅ ticketing.rs: Extracted (ticketing system)
//! - ✅ compliance.rs: Extracted (compliance monitoring)
//! - ✅ notifications.rs: Extracted (notifications)
//! - ✅ linkedin.rs: Extracted (LinkedIn tables)
//! - ✅ healthkit.rs: Extracted (HealthKit tables)
//! - ✅ apple_music.rs: Extracted (Apple Music tables)
//! - ✅ correlation.rs: Extracted (correlation tables)
//! - ✅ platform.rs: Extracted (platform tables)
//! - ✅ indexes.rs: Extracted (all indexes)
//!
//! NOTE: blockchain_hash is a column in multiple tables, not a separate table group.
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
pub mod notifications;
pub mod reports;
pub mod ticketing;

// Data source modules
pub mod apple_music;
pub mod correlation;
pub mod healthkit;
pub mod linkedin;

// Platform and infrastructure modules
pub mod indexes;
pub mod platform;

// Re-exports for convenience
pub use ai::initialize_ai_tables;
pub use apple_music::initialize_apple_music_tables;
pub use compliance::initialize_compliance_tables;
pub use core::{initialize_core_indexes, initialize_core_tables};
pub use correlation::initialize_correlation_tables;
pub use financial::initialize_financial_tables;
pub use healthkit::initialize_healthkit_tables;
pub use identity::initialize_identity_tables;
pub use indexes::initialize_all_indexes;
pub use linkedin::initialize_linkedin_tables;
pub use notifications::initialize_notification_tables;
pub use payments::initialize_payment_tables;
pub use plaid::initialize_all_plaid_tables;
pub use platform::initialize_platform_tables;
pub use reports::initialize_all_reports_tables;
pub use ticketing::initialize_ticketing_tables;
pub use webhook::initialize_webhook_tables;
pub use workflow::initialize_workflow_tables;

use anyhow::Result;
use libsql::Connection;

/// Schema category for tracking which tables have been initialized
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaCategory {
    Core,
    Financial,
    Identity,
    Plaid,
    Payments,
    Reports,
    Ticketing,
    Compliance,
    Notifications,
    HealthKit,
    LinkedIn,
    Correlation,
    AppleMusic,
    Platform,
    Ai,
    Workflow,
    Webhook,
    Indexes,
}

impl SchemaCategory {
    /// Get all schema categories
    pub fn all() -> &'static [SchemaCategory] {
        &[
            SchemaCategory::Core,
            SchemaCategory::Financial,
            SchemaCategory::Identity,
            SchemaCategory::Plaid,
            SchemaCategory::Payments,
            SchemaCategory::Reports,
            SchemaCategory::Ticketing,
            SchemaCategory::Compliance,
            SchemaCategory::Notifications,
            SchemaCategory::HealthKit,
            SchemaCategory::LinkedIn,
            SchemaCategory::Correlation,
            SchemaCategory::AppleMusic,
            SchemaCategory::Platform,
            SchemaCategory::Ai,
            SchemaCategory::Workflow,
            SchemaCategory::Webhook,
            SchemaCategory::Indexes,
        ]
    }

    /// Get the category name
    pub fn name(&self) -> &'static str {
        match self {
            SchemaCategory::Core => "core",
            SchemaCategory::Financial => "financial",
            SchemaCategory::Identity => "identity",
            SchemaCategory::Plaid => "plaid",
            SchemaCategory::Payments => "payments",
            SchemaCategory::Reports => "reports",
            SchemaCategory::Ticketing => "ticketing",
            SchemaCategory::Compliance => "compliance",
            SchemaCategory::Notifications => "notifications",
            SchemaCategory::HealthKit => "healthkit",
            SchemaCategory::LinkedIn => "linkedin",
            SchemaCategory::Correlation => "correlation",
            SchemaCategory::AppleMusic => "apple_music",
            SchemaCategory::Platform => "platform",
            SchemaCategory::Ai => "ai",
            SchemaCategory::Workflow => "workflow",
            SchemaCategory::Webhook => "webhook",
            SchemaCategory::Indexes => "indexes",
        }
    }
}

/// Initialize all schema tables from extracted modules
///
/// This function initializes all tables that have been extracted into schema modules.
/// It provides a complete schema initialization using the modular approach.
///
/// Table counts by module:
/// - Core: 6 tables (user_profile, user_preferences, api_keys, webauthn_credentials, kilt_dids, auth_tokens)
/// - Financial: 3 tables (accounts, transactions, balances)
/// - Identity: 2 tables (identity_verification, verified_credentials)
/// - AI: 3 tables (ai_conversations, ai_messages, uploaded_files)
/// - Workflow: 1 table (workflows)
/// - Webhook: 2 tables (webhook_events, notifications)
/// - Plaid: 20 tables (items, accounts, identities, assets, etc.)
/// - Payments: 5 tables (customers, funding_sources, payments, etc.)
/// - Reports: 8 tables (reports, scores, offers, disputes, etc.)
/// - Ticketing: 4 tables (tickets, ticket_comments, etc.)
/// - Compliance: 4 tables (compliance_scans, compliance_rules, etc.)
/// - Notifications: 2 tables (webhook_events, notifications - shared with webhook)
/// - LinkedIn: 6 tables (linkedin_profiles, etc.)
/// - HealthKit: 6 tables (healthkit_profiles, etc.)
/// - Apple Music: 6 tables (apple_music_profiles, etc.)
/// - Correlation: 3 tables (correlation_preferences, etc.)
/// - Platform: 4 tables (data_approval_hashes, referrals, etc.)
///
/// HARDCODED_SCHEMA: 83 unique tables total across all modules (verified 2025-12-31)
/// Note: Some tables appear in multiple modules but SQLite IF NOT EXISTS handles deduplication.
pub async fn initialize_all_schema_tables(conn: &Connection) -> Result<()> {
    // Enable foreign key constraints first
    conn.execute("PRAGMA foreign_keys = ON", ()).await?;

    // Core tables (user_profile must be first - referenced by other tables)
    initialize_core_tables(conn).await?;
    initialize_core_indexes(conn).await?;

    // Financial tables
    initialize_financial_tables(conn).await?;

    // Identity tables
    initialize_identity_tables(conn).await?;

    // AI tables
    initialize_ai_tables(conn).await?;

    // Workflow tables
    initialize_workflow_tables(conn).await?;

    // Webhook tables
    initialize_webhook_tables(conn).await?;

    // Plaid product tables
    initialize_all_plaid_tables(conn).await?;

    // Payment tables
    initialize_payment_tables(conn).await?;

    // Business logic tables
    initialize_all_reports_tables(conn).await?;
    initialize_ticketing_tables(conn).await?;
    initialize_compliance_tables(conn).await?;
    initialize_notification_tables(conn).await?;

    // Data source tables
    initialize_linkedin_tables(conn).await?;
    initialize_healthkit_tables(conn).await?;
    initialize_apple_music_tables(conn).await?;
    initialize_correlation_tables(conn).await?;

    // Platform tables
    initialize_platform_tables(conn).await?;

    // All remaining indexes (organized by category)
    initialize_all_indexes(conn).await?;

    Ok(())
}

/// Initialize extracted schema tables (legacy function for compatibility)
///
/// This function initializes only the originally extracted tables.
/// For full schema initialization, use initialize_all_schema_tables().
pub async fn initialize_extracted_tables(conn: &Connection) -> Result<()> {
    initialize_core_tables(conn).await?;
    initialize_financial_tables(conn).await?;
    initialize_identity_tables(conn).await?;
    initialize_ai_tables(conn).await?;
    initialize_workflow_tables(conn).await?;
    initialize_webhook_tables(conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_category_all() {
        let categories = SchemaCategory::all();
        // 18 categories: Core, Financial, Identity, Plaid, Payments, Reports,
        // Ticketing, Compliance, Notifications, HealthKit, LinkedIn, Correlation,
        // AppleMusic, Platform, Ai, Workflow, Webhook, Indexes
        assert_eq!(categories.len(), 18);
    }

    #[test]
    fn test_schema_category_names() {
        assert_eq!(SchemaCategory::Core.name(), "core");
        assert_eq!(SchemaCategory::Financial.name(), "financial");
        assert_eq!(SchemaCategory::Plaid.name(), "plaid");
        assert_eq!(SchemaCategory::Payments.name(), "payments");
        assert_eq!(SchemaCategory::Reports.name(), "reports");
        assert_eq!(SchemaCategory::Indexes.name(), "indexes");
    }

    #[test]
    fn test_schema_category_equality() {
        assert_eq!(SchemaCategory::Core, SchemaCategory::Core);
        assert_ne!(SchemaCategory::Core, SchemaCategory::Financial);
    }
}
