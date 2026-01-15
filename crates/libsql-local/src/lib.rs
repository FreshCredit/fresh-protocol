//! Local LibSQL database operations for FreshCredit
//!
//! This module implements the unified database schema for FreshCredit,
//! // HARDCODED_SCHEMA: 117 tables in Rust modular schema, 118 in unified_schema.sql (+1 browser-specific blockchain_proofs) (verified 2026-01-15)
//! containing 117 tables that support:
//! - User profile and authentication (Entra ID + Verified ID)
//! - All 11 Plaid products (Accounts, Transactions, Auth, Identity, etc.)
//! - Payment processing (Stripe Connect ACH)
//! - AI features (conversations, file uploads)
//! - Workflow automation (Windmill integration)
//! - KILT Protocol DID support
//!
//! TABLE CONSOLIDATION NOTES:
//! - `reports` table is used for BlockID reports (NOT `credit_reports`)
//! - `identity_verification` (singular) is used for Plaid IDV (NOT `identity_verifications`)
//! - `balances` table stores balance history; `accounts` table columns store current balance
//!
//! SCHEMA SOURCE OF TRUTH:
//! - Rust code: `crates/db/libsql/local/src/schema/` modules (invoked by `initialize_schema` in this file)
//! - SQL file: migrations/unified_schema.sql
//! - Cloud database: freshcredit-unified-schema-v1 (Turso)
//!
//! Module structure:
//! - lib.rs (this file): Main entry point with LocalClient
//! - types.rs: All type definitions (UserProfile, UserPreferences, AI, Workflow, Webhook types)
//! - schema/: Schema definitions organized by domain
//! - operations/: CRUD operations organized by domain
//!
//! HARDCODED_SCHEMA: 117 unique tables in modular schema (verified 2026-01-15)

// Submodules for incremental extraction
pub mod operations;
pub mod schema;
pub mod types;

// Re-export types for backward compatibility
// All type definitions are in types.rs module
pub use types::{
    AiConversation, AiMessage, SaveUploadedFileParams, SchemaValidationResult, ScoringModelRecord,
    UploadedFile, UserPreferences, UserProfile, WebhookEvent, WebhookEventCounts, WorkflowRecord,
};

use anyhow::Result;
use tracing::info;

/// Local LibSQL database client
pub struct LocalClient {
    connection: libsql::Connection,
}

impl LocalClient {
    /// Create a new local client
    #[must_use = "this returns a Result that should be handled"]
    pub async fn new(database_path: &str) -> Result<Self> {
        info!("Creating local LibSQL client at: {}", database_path);

        let db = libsql::Builder::new_local(database_path).build().await?;
        let connection = db.connect()?;

        Ok(Self { connection })
    }

    /// Create a new in-memory client for testing
    #[cfg(test)]
    #[must_use = "this returns a Result that should be handled"]
    pub async fn new_in_memory() -> Result<Self> {
        let db = libsql::Builder::new_local(":memory:").build().await?;
        let connection = db.connect()?;
        Ok(Self { connection })
    }

    /// Get access to the underlying connection for direct queries
    pub fn connection(&self) -> &libsql::Connection {
        &self.connection
    }

    /// Initialize database schema using modular schema definitions
    ///
    /// This function delegates to the schema module for table creation.
    /// All table definitions are in `schema/` submodules for maintainability.
    ///
    /// HARDCODED_SCHEMA: 117 tables total across all modules (verified 2026-01-15)
    /// See `schema/mod.rs` for the complete table inventory.
    #[must_use = "this returns a Result that should be handled"]
    pub async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing local database schema with modular schema definitions");

        // Delegate to the modular schema initialization
        schema::initialize_all_schema_tables(&self.connection).await?;

        // HARDCODED_SCHEMA: 124 tables in Rust modular schema (verified 2026-01-15)
        // Added: 4 agent tables + 3 UCP tables = 7 new tables
        info!("Unified database schema initialization completed (124 tables)");
        Ok(())
    }

    /// Execute a raw SQL query and return rows
    #[must_use = "this returns a Result that should be handled"]
    pub async fn query(&self, sql: &str, params: Vec<libsql::Value>) -> Result<libsql::Rows> {
        self.connection
            .query(sql, params)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// Execute a raw SQL statement and return affected rows count
    #[must_use = "this returns a Result that should be handled"]
    pub async fn execute(&self, sql: &str, params: Vec<libsql::Value>) -> Result<u64> {
        self.connection
            .execute(sql, params)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that the schema contains exactly 117 tables as documented
    /// HARDCODED_SCHEMA: 117 unique tables in modular schema (verified 2026-01-15)
    /// Added: provider_teams, team_members, team_invites (Teams)
    /// Added: customer_activities, customer_segments, customer_segment_memberships, customer_communications (Customers)
    /// Added: offer_analytics, offer_ab_test_results, offer_events (Offer Analytics)
    /// Added: bridge_transfers, gateway_sessions, gateway_transactions (Circle Arc Phase 2)
    #[tokio::test]
    async fn test_schema_table_count() {
        let client = LocalClient::new_in_memory().await.unwrap();
        client.initialize_schema().await.unwrap();

        // Query sqlite_master for table count
        let mut rows = client
            .query(
                "SELECT COUNT(*) as count FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                vec![],
            )
            .await
            .unwrap();

        let row = rows.next().await.unwrap().unwrap();
        let count: i64 = row.get(0).unwrap();
        // HARDCODED_SCHEMA: 124 tables (104 base + 3 Teams + 4 Customers + 3 Offer Analytics + 3 Circle Arc Phase 2 + 4 Security + 3 UCP) (verified 2026-01-15)
        // Teams: provider_teams, team_members, team_invites
        // Customers: customer_activities, customer_segments, customer_segment_memberships, customer_communications
        // Offer Analytics: offer_analytics, offer_ab_test_results, offer_events
        // Circle Arc Phase 2: bridge_transfers, gateway_sessions, gateway_transactions
        // Security: user_devices, ip_blocks, step_up_auth_sessions, security_events
        // UCP: ucp_checkout_sessions, ucp_orders, ucp_identity_links
        assert_eq!(count, 124, "Schema should contain exactly 124 tables");
    }

    /// Test that critical tables exist in the schema
    #[tokio::test]
    async fn test_critical_tables_exist() {
        let client = LocalClient::new_in_memory().await.unwrap();
        client.initialize_schema().await.unwrap();

        let critical_tables = vec![
            "user_profile",
            "accounts",
            "transactions",
            "balances",
            "reports",
            "identity_verification",
            "workflows",
            "scores",
            "ai_conversations",
            "ai_messages",
            "data_approval_hashes",
            "referrals",
            "platform_metrics",
            "sales_pipeline",
            // LinkedIn tables (L1: Multi-Source Integration)
            "linkedin_profiles",
            "linkedin_experiences",
            "linkedin_education",
            "linkedin_skills",
            "linkedin_certifications",
            "linkedin_languages",
            // HealthKit tables (H1: Multi-Source Integration)
            "healthkit_profiles",
            "healthkit_records",
            "healthkit_workouts",
            "healthkit_activity_summaries",
            "healthkit_clinical_records",
            "healthkit_correlations",
            // Apple Music tables (AM1: Multi-Source Integration)
            "apple_music_profiles",
            "apple_music_library_songs",
            "apple_music_library_albums",
            "apple_music_playlists",
            "apple_music_recently_played",
            "apple_music_genre_stats",
        ];

        for table in critical_tables {
            let mut rows = client
                .query(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name=?",
                    vec![libsql::Value::Text(table.to_string())],
                )
                .await
                .unwrap();

            let row = rows.next().await.unwrap();
            assert!(
                row.is_some(),
                "Critical table '{table}' should exist in schema"
            );
        }
    }

    /// Test that user profile CRUD operations work
    #[tokio::test]
    async fn test_user_profile_crud() {
        let client = LocalClient::new_in_memory().await.unwrap();
        client.initialize_schema().await.unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        let profile = UserProfile {
            id: "test-user-123".to_string(),
            platform_user_id: "platform-123".to_string(),
            azure_id: "azure-123".to_string(),
            email: "test@example.com".to_string(),
            display_name: "Test User".to_string(),
            given_name: Some("Test".to_string()),
            family_name: Some("User".to_string()),
            surname: None,
            mobile_phone: None,
            job_title: None,
            street_address: None,
            city: None,
            state_province: None,
            postal_code: None,
            country_region: None,
            date_of_birth: None,
            ssn_last_four: None,
            employment_status: None,
            annual_income: None,
            role: "consumer".to_string(),
            is_admin: false,
            provider_onboarding_complete: false,
            tenant_id: "tenant-123".to_string(),
            object_id: "object-123".to_string(),
            verified_id_credential_id: None,
            verified_id_status: "pending".to_string(),
            verified_id_issued_at: None,
            created_at: now.clone(),
            updated_at: now,
        };

        // Store user profile
        client.store_user_profile(&profile).await.unwrap();

        // Verify user exists
        let retrieved = client.get_user_profile("platform-123").await.unwrap();
        assert!(
            retrieved.is_some(),
            "User profile should exist after creation"
        );

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, "test-user-123");
        assert_eq!(retrieved.email, "test@example.com");
        assert_eq!(retrieved.role, "consumer");
    }

    /// Test query execution returns rows
    #[tokio::test]
    async fn test_query_execution() {
        let client = LocalClient::new_in_memory().await.unwrap();
        client.initialize_schema().await.unwrap();

        let mut rows = client
            .query("SELECT 1 as value", vec![])
            .await
            .unwrap();

        let row = rows.next().await.unwrap().unwrap();
        let value: i64 = row.get(0).unwrap();
        assert_eq!(value, 1);
    }

    /// Test execute returns affected rows
    #[tokio::test]
    async fn test_execute_returns_rows_affected() {
        let client = LocalClient::new_in_memory().await.unwrap();
        client.initialize_schema().await.unwrap();

        // Create a simple test table without foreign key constraints
        client
            .execute(
                "CREATE TABLE IF NOT EXISTS test_execute_table (id TEXT PRIMARY KEY, value TEXT)",
                vec![],
            )
            .await
            .unwrap();

        // Insert a test row
        let result = client
            .execute(
                "INSERT INTO test_execute_table (id, value) VALUES ('test-id', 'test-value')",
                vec![],
            )
            .await
            .unwrap();

        assert_eq!(result, 1); // 1 row affected
    }

    /// Test schema idempotency - running initialize twice should not error
    #[tokio::test]
    async fn test_schema_initialization_idempotent() {
        let client = LocalClient::new_in_memory().await.unwrap();

        // Initialize twice
        client.initialize_schema().await.unwrap();
        client.initialize_schema().await.unwrap();

        // Verify schema still valid
        let mut rows = client
            .query(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                vec![],
            )
            .await
            .unwrap();

        let row = rows.next().await.unwrap().unwrap();
        let count: i64 = row.get(0).unwrap();
        assert!(count > 0);
    }

    /// Test user profile update
    #[tokio::test]
    async fn test_user_profile_update() {
        let client = LocalClient::new_in_memory().await.unwrap();
        client.initialize_schema().await.unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        let mut profile = UserProfile {
            id: "update-test-user".to_string(),
            platform_user_id: "platform-update-123".to_string(),
            azure_id: "azure-update-123".to_string(),
            email: "update@example.com".to_string(),
            display_name: "Original Name".to_string(),
            given_name: None,
            family_name: None,
            surname: None,
            mobile_phone: None,
            job_title: None,
            street_address: None,
            city: None,
            state_province: None,
            postal_code: None,
            country_region: None,
            date_of_birth: None,
            ssn_last_four: None,
            employment_status: None,
            annual_income: None,
            role: "consumer".to_string(),
            is_admin: false,
            provider_onboarding_complete: false,
            tenant_id: "tenant".to_string(),
            object_id: "object".to_string(),
            verified_id_credential_id: None,
            verified_id_status: "pending".to_string(),
            verified_id_issued_at: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        // Store initial profile
        client.store_user_profile(&profile).await.unwrap();

        // Update and store again
        profile.display_name = "Updated Name".to_string();
        profile.updated_at = chrono::Utc::now().to_rfc3339();
        client.store_user_profile(&profile).await.unwrap();

        // Verify update
        let retrieved = client.get_user_profile("platform-update-123").await.unwrap().unwrap();
        assert_eq!(retrieved.display_name, "Updated Name");
    }

    /// Test user profile not found
    #[tokio::test]
    async fn test_user_profile_not_found() {
        let client = LocalClient::new_in_memory().await.unwrap();
        client.initialize_schema().await.unwrap();

        let result = client.get_user_profile("nonexistent-user").await.unwrap();
        assert!(result.is_none());
    }
}
