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

use anyhow::Result;
use libsql::Connection;

/// Helper to try creating an index, ignoring "no such column" errors
/// This is needed because embedded replicas may sync from Turso cloud
/// which could have an older schema without certain columns.
pub async fn try_create_index(conn: &Connection, sql: &str) -> Result<()> {
    match conn.execute(sql, ()).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let err_msg = e.to_string();
            // Ignore "no such column" and "no such table" errors
            // These happen when syncing from older cloud schemas
            if err_msg.contains("no such column") || err_msg.contains("no such table") {
                tracing::debug!(
                    "Skipping index creation (column/table not in synced schema): {sql}"
                );
                Ok(())
            } else {
                Err(anyhow::anyhow!("{e}"))
            }
        }
    }
}

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
    Ip,
    Publications,
    Correlation,
    AppleMusic,
    Platform,
    Teams,
    Customers,
    Ai,
    Workflow,
    Webhook,
    Security,
    Governance,
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
            SchemaCategory::Ip,
            SchemaCategory::Publications,
            SchemaCategory::Correlation,
            SchemaCategory::AppleMusic,
            SchemaCategory::Platform,
            SchemaCategory::Teams,
            SchemaCategory::Customers,
            SchemaCategory::Ai,
            SchemaCategory::Workflow,
            SchemaCategory::Webhook,
            SchemaCategory::Security,
            SchemaCategory::Governance,
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
            SchemaCategory::Ip => "ip",
            SchemaCategory::Publications => "publications",
            SchemaCategory::Correlation => "correlation",
            SchemaCategory::AppleMusic => "apple_music",
            SchemaCategory::Platform => "platform",
            SchemaCategory::Teams => "teams",
            SchemaCategory::Customers => "customers",
            SchemaCategory::Ai => "ai",
            SchemaCategory::Workflow => "workflow",
            SchemaCategory::Webhook => "webhook",
            SchemaCategory::Security => "security",
            SchemaCategory::Governance => "governance",
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
/// - Payments: 8 tables (customers, funding_sources, payments, crypto_wallets, crypto_payments, arc_receipts, etc.)
/// - Reports: 8 tables (reports, scores, offers, disputes, etc.)
/// - Ticketing: 4 tables (tickets, ticket_comments, etc.)
/// - Compliance: 4 tables (compliance_scans, compliance_rules, etc.)
/// - Notifications: 2 tables (webhook_events, notifications - shared with webhook)
/// - LinkedIn: 6 tables (linkedin_profiles, etc.)
/// - HealthKit: 6 tables (healthkit_profiles, etc.)
/// - IP: 5 tables (ip_records, ip_claims, ip_evidence, ip_events, ip_disputes)
/// - Publications: 7 tables (publication_records, publication_claims, orcid_connections, publication_evidence, publication_events, publication_disputes, publication_shares)
/// - Apple Music: 6 tables (apple_music_profiles, etc.)
/// - Correlation: 3 tables (correlation_preferences, etc.)
/// - Platform: 4 tables (data_approval_hashes, referrals, etc.)
/// - Teams: 3 tables (provider_teams, team_members, team_invites)
/// - Customers: 4 tables (customer_activities, customer_segments, customer_segment_memberships, customer_communications)
/// - Security: 5 tables (ip_blocks, rate_limit_events, step_up_auth_requests, user_devices, compliance_digests)
///
/// HARDCODED_SCHEMA: 133 unique tables total across all modules (verified 2026-01-18)
/// Core: 6 tables (user_profile, user_preferences, api_keys, webauthn_credentials, kilt_dids, auth_tokens)
/// Financial: 3 tables | Identity: 2 tables | AI: 7 tables | Workflow: 1 table | Webhook: 2 tables
/// Plaid: 20 tables | Payments: 11 tables | Reports: 11 tables | Ticketing: 4 tables | Compliance: 5 tables
/// LinkedIn: 6 tables | HealthKit: 6 tables | IP: 5 tables | Publications: 7 tables | Apple Music: 6 tables
/// Correlation: 3 tables | Platform: 3 tables | Teams: 3 tables | Customers: 4 tables | Security: 4 tables
/// UCP: 3 tables | Agent: 6 tables | Governance: 6 tables
/// Note: SQLite IF NOT EXISTS handles deduplication for tables appearing in multiple modules.
/// Validates schema initialization by tracking which categories are initialized
/// ARCH-007: Schema validation tracking for startup validation logging
#[derive(Debug)]
#[allow(dead_code)]
struct SchemaValidation {
    category: &'static str,
    tables_initialized: bool,
}

/// Initialize all schema tables with validation logging
/// ARCH-007: Tracks schema initialization for startup validation
pub async fn initialize_all_schema_tables(conn: &Connection) -> Result<()> {
    use tracing::info;

    // Disable foreign key constraints during schema initialization
    // This allows synced data from Turso cloud to load even if referenced rows
    // arrive in a different order. We re-enable at the end.
    conn.execute("PRAGMA foreign_keys = OFF", ()).await?;

    let mut validations = Vec::new();

    // Core tables (user_profile must be first - referenced by other tables)
    initialize_core_tables(conn).await?;
    initialize_core_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "core",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: core");

    // Financial tables
    initialize_financial_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "financial",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: financial");

    // Identity tables
    initialize_identity_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "identity",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: identity");

    // AI tables
    initialize_ai_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "ai",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: ai");

    // Workflow tables
    initialize_workflow_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "workflow",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: workflow");

    // Webhook tables
    initialize_webhook_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "webhook",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: webhook");

    // Plaid product tables
    initialize_all_plaid_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "plaid",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: plaid");

    // Payment tables
    initialize_payment_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "payments",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: payments");

    // Business logic tables
    initialize_all_reports_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "reports",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: reports");

    initialize_ticketing_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "ticketing",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: ticketing");

    initialize_compliance_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "compliance",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: compliance");

    initialize_notification_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "notifications",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: notifications");

    // Data source tables
    initialize_linkedin_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "linkedin",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: linkedin");

    initialize_healthkit_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "healthkit",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: healthkit");

    initialize_ip_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "ip",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: ip");

    initialize_publication_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "publications",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: publications");

    initialize_apple_music_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "apple_music",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: apple_music");

    initialize_correlation_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "correlation",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: correlation");

    // Platform tables
    initialize_platform_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "platform",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: platform");

    // Teams tables (provider_teams, team_members, team_invites)
    initialize_teams_tables(conn).await?;
    initialize_teams_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "teams",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: teams");

    // Customer management tables (customer_activities, customer_segments, etc.)
    initialize_customer_tables(conn).await?;
    initialize_customer_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "customers",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: customers");

    // Security tables (ip_blocks, rate_limit_events, step_up_auth, user_devices, compliance_digests)
    initialize_security_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "security",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: security");

    // Agent tables (agent_bindings, agent_memories, agent_interactions, agent_audit_events)
    initialize_agent_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "agent",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: agent");

    // UCP tables (ucp_checkout_sessions, ucp_orders, ucp_identity_links)
    // COMPLIANCE: AGENT-004 - All UCP tables include user_id for data access control
    initialize_ucp_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "ucp",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: ucp");

    // Governance tables (governance_proposals, governance_votes, governance_delegations, etc.)
    // COMPLIANCE: §7 - Uses neutral governance terminology
    initialize_governance_tables(conn).await?;
    initialize_governance_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "governance",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: governance");

    // All remaining indexes (organized by category)
    initialize_all_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "indexes",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: indexes");

    // Final validation summary
    info!(
        "[ARCH-007] Schema initialization complete: {} categories validated",
        validations.len()
    );

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
        // 24 categories: Core, Financial, Identity, Plaid, Payments, Reports,
        // Ticketing, Compliance, Notifications, HealthKit, LinkedIn, Ip, Publications,
        // Correlation, AppleMusic, Platform, Teams, Customers, Ai, Workflow, Webhook, Security, Governance, Indexes
        assert_eq!(categories.len(), 24);
    }

    #[test]
    fn test_schema_category_names() {
        assert_eq!(SchemaCategory::Core.name(), "core");
        assert_eq!(SchemaCategory::Financial.name(), "financial");
        assert_eq!(SchemaCategory::Plaid.name(), "plaid");
        assert_eq!(SchemaCategory::Payments.name(), "payments");
        assert_eq!(SchemaCategory::Reports.name(), "reports");
        assert_eq!(SchemaCategory::Publications.name(), "publications");
        assert_eq!(SchemaCategory::Indexes.name(), "indexes");
    }

    #[test]
    fn test_schema_category_equality() {
        assert_eq!(SchemaCategory::Core, SchemaCategory::Core);
        assert_ne!(SchemaCategory::Core, SchemaCategory::Financial);
    }
}
