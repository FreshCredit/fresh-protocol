//! Database schema definitions for FreshCredit unified schema
//!
//! SCHEMA SOURCE OF TRUTH:
//! - Rust code: crates/db/libsql/local/src/lib.rs (initialize_schema function)
//! - SQL file: migrations/unified_schema.sql
//! - Cloud database: freshcredit-unified-schema-v1 (Turso)
//!
//! Module structure:
//! - mod.rs (this file): Module exports, SchemaCategory enum, unified initialization
//! - core.rs: Core tables (user_profile, auth_tokens, user_preferences) + 4 indexes
//! - financial.rs: Financial tables (accounts, transactions, balances) + 6 indexes
//! - identity.rs: Identity tables (identity_verification, verified_credentials) + 5 indexes
//! - ai.rs: AI tables (ai_conversations, ai_messages, uploaded_files) + 3 indexes
//! - workflow.rs: Workflow tables (workflows) + 3 indexes
//! - webhook.rs: Webhook tables (webhook_events, notifications) + 5 indexes
//!
//! REFACTORING STATUS:
//! - ✅ core.rs: Extracted (user_profile, auth_tokens, user_preferences)
//! - ✅ financial.rs: Extracted (accounts, transactions, balances)
//! - ✅ identity.rs: Extracted (identity_verification, verified_credentials)
//! - ✅ ai.rs: Extracted (ai_conversations, ai_messages, uploaded_files)
//! - ✅ workflow.rs: Extracted (workflows)
//! - ✅ webhook.rs: Extracted (webhook_events, notifications)
//! - ⏳ healthkit.rs: Pending (healthkit_* tables - 6 tables)
//! - ⏳ linkedin.rs: Pending (linkedin_* tables - 6 tables)
//! - ⏳ correlation.rs: Pending (correlation_* tables - 3 tables)
//! - ⏳ apple_music.rs: Pending (apple_music_* tables - 6 tables)
//!
//! NOTE: blockchain_hash is a column in multiple tables, not a separate table group.
//! NOTE: staging tables are in the staging crate, not local schema.
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

pub mod ai;
pub mod core;
pub mod financial;
pub mod identity;
pub mod webhook;
pub mod workflow;

pub use ai::initialize_ai_tables;
pub use core::initialize_core_tables;
pub use financial::initialize_financial_tables;
pub use identity::initialize_identity_tables;
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
    Blockchain,
    Staging,
    HealthKit,
    LinkedIn,
    Correlation,
    AppleMusic,
    Ai,
    Workflow,
    Webhook,
}

impl SchemaCategory {
    /// Get all schema categories
    pub fn all() -> &'static [SchemaCategory] {
        &[
            SchemaCategory::Core,
            SchemaCategory::Financial,
            SchemaCategory::Identity,
            SchemaCategory::Blockchain,
            SchemaCategory::Staging,
            SchemaCategory::HealthKit,
            SchemaCategory::LinkedIn,
            SchemaCategory::Correlation,
            SchemaCategory::AppleMusic,
            SchemaCategory::Ai,
            SchemaCategory::Workflow,
            SchemaCategory::Webhook,
        ]
    }

    /// Get the category name
    pub fn name(&self) -> &'static str {
        match self {
            SchemaCategory::Core => "core",
            SchemaCategory::Financial => "financial",
            SchemaCategory::Identity => "identity",
            SchemaCategory::Blockchain => "blockchain",
            SchemaCategory::Staging => "staging",
            SchemaCategory::HealthKit => "healthkit",
            SchemaCategory::LinkedIn => "linkedin",
            SchemaCategory::Correlation => "correlation",
            SchemaCategory::AppleMusic => "apple_music",
            SchemaCategory::Ai => "ai",
            SchemaCategory::Workflow => "workflow",
            SchemaCategory::Webhook => "webhook",
        }
    }
}

/// Initialize extracted schema tables
///
/// This function initializes the tables that have been extracted from lib.rs.
/// The remaining tables are still initialized by initialize_schema() in lib.rs.
///
/// Extracted modules:
/// - core: user_profile, auth_tokens, user_preferences
/// - financial: accounts, transactions, balances
/// - identity: identity_verification, verified_credentials
/// - ai: ai_conversations, ai_messages, uploaded_files
/// - workflow: workflows
/// - webhook: webhook_events, notifications
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
        assert_eq!(categories.len(), 12);
    }

    #[test]
    fn test_schema_category_names() {
        assert_eq!(SchemaCategory::Core.name(), "core");
        assert_eq!(SchemaCategory::Financial.name(), "financial");
        assert_eq!(SchemaCategory::Blockchain.name(), "blockchain");
    }

    #[test]
    fn test_schema_category_equality() {
        assert_eq!(SchemaCategory::Core, SchemaCategory::Core);
        assert_ne!(SchemaCategory::Core, SchemaCategory::Financial);
    }
}
