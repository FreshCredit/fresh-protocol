//! Local LibSQL database operations for FreshCredit
//!
//! This module implements the unified database schema for FreshCredit,
//! // HARDCODED_SCHEMA: 64 tables - update if schema changes
//! containing 64 tables that support:
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
//! - Rust code: crates/db/libsql/local/src/lib.rs (this file, initialize_schema function)
//! - SQL file: migrations/unified_schema.sql
//! - Cloud database: freshcredit-unified-schema-v1 (Turso)
//!
//! Module structure (REFACTORING IN PROGRESS):
//! - lib.rs (this file): Main entry point with LocalClient and all operations
//! - schema/: Schema definitions organized by domain (target structure)
//! - operations/: CRUD operations organized by domain (target structure)
//!
//! Schema Version: unified-v1 (2025-12-05)

// Submodules for incremental extraction
pub mod schema;
pub mod operations;

use anyhow::Result;
use freshcredit_types::{FinancialReport, FreshCreditResult, UserId};
use serde::{Deserialize, Serialize};
use tracing::info;

/// User profile for database storage (unified schema)
/// Combines Entra ID claims with extended profile and Verified ID fields
/// P0p: Added is_admin for first provider user admin rule (§27.4)
/// P0g: Added provider_onboarding_complete for nav visibility (§28.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub platform_user_id: String,
    pub azure_id: String,
    pub email: String,
    pub display_name: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub surname: Option<String>,
    pub mobile_phone: Option<String>,
    pub job_title: Option<String>,
    pub street_address: Option<String>,
    pub city: Option<String>,
    pub state_province: Option<String>,
    pub postal_code: Option<String>,
    pub country_region: Option<String>,
    pub date_of_birth: Option<String>,
    pub ssn_last_four: Option<String>,
    pub employment_status: Option<String>,
    pub annual_income: Option<i32>,
    pub role: String,
    /// P0p: First provider user is admin by default (§27.4)
    #[serde(default)]
    pub is_admin: bool,
    /// P0g: Provider onboarding completion status (§28.1)
    #[serde(default)]
    pub provider_onboarding_complete: bool,
    pub tenant_id: String,
    pub object_id: String,
    pub verified_id_credential_id: Option<String>,
    pub verified_id_status: String,
    pub verified_id_issued_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Schema validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationResult {
    pub is_valid: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub checked_at: String,
}

/// User preferences for toggle states
/// P0g: Added onboarding dismissal fields for §27.3 onboarding flow rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub ai_agent_enabled: Option<bool>,
    pub ai_feedback_enabled: Option<bool>,
    pub ai_offers_enabled: Option<bool>,
    pub ai_lenders_enabled: Option<bool>,
    pub cloud_sync_enabled: Option<bool>,
    pub blockchain_enabled: Option<bool>,
    pub email_notifications_enabled: Option<bool>,
    pub kilt_did_enabled: Option<bool>,
    /// AI mode preference: "auto" (default), "cloud", or "local"
    pub ai_mode: Option<String>,
    /// Mock data mode for internal users testing flows
    /// When enabled, pages display prefilled mock data without database persistence
    /// Only available for @freshcredit.com internal team members
    pub mock_data_enabled: Option<bool>,
    /// P0g: Onboarding completed flag (§27.3)
    #[serde(default)]
    pub onboarding_completed: Option<bool>,
    /// P0g: Permanent dismissal flag - "Don't Show Again" (§27.3)
    #[serde(default)]
    pub onboarding_permanently_dismissed: Option<bool>,
    /// P0g: Reminder dismissal timestamp - "Remind Later" re-prompt after 7 days (§27.3)
    #[serde(default)]
    pub onboarding_reminder_dismissed_until: Option<String>,
    /// P0g: Plaid connection skipped flag (§27.3)
    #[serde(default)]
    pub plaid_connection_skipped: Option<bool>,
    /// P0g: Plaid reminder dismissal timestamp (§27.3)
    #[serde(default)]
    pub plaid_reminder_dismissed_until: Option<String>,
}

/// Uploaded file for AI multimodal input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadedFile {
    pub id: String,
    pub user_id: String,
    pub filename: String,
    pub mime_type: String,
    pub file_size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_data: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_analysis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Parameters for saving an uploaded file
///
/// Consolidates function arguments to avoid clippy::too_many_arguments
#[derive(Debug, Clone)]
pub struct SaveUploadedFileParams<'a> {
    pub user_id: &'a str,
    pub filename: &'a str,
    pub mime_type: &'a str,
    pub file_size: i64,
    pub file_data: Option<Vec<u8>>,
    pub text_content: Option<&'a str>,
    pub conversation_id: Option<&'a str>,
}

/// Local LibSQL database client
pub struct LocalClient {
    connection: libsql::Connection,
}

impl LocalClient {
    /// Create a new local client
    pub async fn new(database_path: &str) -> Result<Self> {
        info!("Creating local LibSQL client at: {}", database_path);

        let db = libsql::Builder::new_local(database_path).build().await?;
        let connection = db.connect()?;

        Ok(Self { connection })
    }

    /// Create a new in-memory client for testing
    #[cfg(test)]
    pub async fn new_in_memory() -> Result<Self> {
        let db = libsql::Builder::new_local(":memory:").build().await?;
        let connection = db.connect()?;
        Ok(Self { connection })
    }

    /// Get access to the underlying connection for direct queries
    pub fn connection(&self) -> &libsql::Connection {
        &self.connection
    }

    /// Initialize database schema using production schema
    pub async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing local database schema with production schema");

        // Enable foreign key constraints first
        self.connection
            .execute("PRAGMA foreign_keys = ON", ())
            .await?;

        // Create user_profile table first (referenced by other tables)
        // P0p: Added is_admin column for first provider user admin rule (§27.4)
        // P0g: Added provider_onboarding_complete for §28.1 nav visibility
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS user_profile (
                id TEXT PRIMARY KEY,
                platform_user_id TEXT NOT NULL,
                azure_id TEXT NOT NULL,
                email TEXT NOT NULL,
                display_name TEXT NOT NULL,
                given_name TEXT,
                family_name TEXT,
                surname TEXT,
                mobile_phone TEXT,
                job_title TEXT,
                street_address TEXT,
                city TEXT,
                state_province TEXT,
                postal_code TEXT,
                country_region TEXT,
                date_of_birth TEXT,
                ssn_last_four TEXT,
                employment_status TEXT,
                annual_income INTEGER,
                role TEXT DEFAULT 'consumer',
                is_admin BOOLEAN DEFAULT FALSE,
                provider_onboarding_complete BOOLEAN DEFAULT FALSE,
                tenant_id TEXT NOT NULL,
                object_id TEXT NOT NULL,
                verified_id_credential_id TEXT,
                verified_id_status TEXT DEFAULT 'pending',
                verified_id_issued_at TEXT,
                last_report_date DATETIME,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )",
                (),
            )
            .await?;

        // P0p: Add is_admin column (migration for existing databases)
        let _ = self
            .connection
            .execute(
                "ALTER TABLE user_profile ADD COLUMN is_admin BOOLEAN DEFAULT FALSE",
                (),
            )
            .await;

        // P0g: Add provider_onboarding_complete column (migration for existing databases)
        let _ = self
            .connection
            .execute(
                "ALTER TABLE user_profile ADD COLUMN provider_onboarding_complete BOOLEAN DEFAULT FALSE",
                (),
            )
            .await;

        // Create accounts table that matches production schema
        // Foreign key disabled to allow account creation before user_profile exists
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                plaid_account_id TEXT,
                plaid_access_token TEXT,
                account_id TEXT UNIQUE,
                institution_id TEXT,
                institution_name TEXT,
                institution_logo TEXT,
                account_name TEXT,
                account_type TEXT NOT NULL,
                account_subtype TEXT,
                mask TEXT,
                balance_available REAL,
                balance_current REAL,
                balance_limit REAL,
                current_balance REAL,
                available_balance REAL,
                currency TEXT DEFAULT 'USD',
                currency_code TEXT DEFAULT 'USD',
                balance REAL DEFAULT 0.0,
                is_funding_source BOOLEAN DEFAULT FALSE,
                is_active BOOLEAN DEFAULT TRUE,
                date_opened DATE,
                credit_limit DECIMAL(12,2),
                blockchain_hash TEXT,
                block_number INTEGER,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                (),
            )
            .await?;

        // Create transactions table that matches production schema
        // Uses composite unique constraint on (account_id, date, amount, name) for deduplication
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                plaid_transaction_id TEXT UNIQUE,
                amount REAL NOT NULL,
                iso_currency_code TEXT DEFAULT 'USD',
                unofficial_currency_code TEXT,
                category TEXT,
                subcategory TEXT,
                personal_finance_category_primary TEXT,
                personal_finance_category_detailed TEXT,
                personal_finance_category_icon TEXT,
                transaction_type TEXT,
                name TEXT NOT NULL,
                merchant_name TEXT,
                merchant_logo_url TEXT,
                pending BOOLEAN DEFAULT FALSE,
                account_owner TEXT,
                date DATE NOT NULL,
                authorized_date DATE,
                location_address TEXT,
                location_city TEXT,
                location_region TEXT,
                location_postal_code TEXT,
                location_country TEXT,
                location_lat REAL,
                location_lon REAL,
                payment_channel TEXT,
                raw_transaction_data TEXT,
                blockchain_hash TEXT,
                block_number INTEGER,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (account_id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create indexes for accounts and transactions tables
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_accounts_user_id ON accounts(user_id)",
                (),
            )
            .await?;

        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_transactions_account_id ON transactions(account_id)",
            (),
        ).await?;

        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date)",
                (),
            )
            .await?;

        // ISSUE 7 FIX: Use plaid_transaction_id UNIQUE constraint on table instead of composite index
        // The composite key (account_id, date, amount, name) caused data loss when multiple transactions
        // on the same day had the same amount and merchant name (e.g., multiple coffee purchases)
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transactions_plaid_txn_id ON transactions(plaid_transaction_id)",
                (),
            )
            .await?;

        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_user_profile_email ON user_profile(email)",
                (),
            )
            .await?;

        // Create auth table for account authentication data
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS auth (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_id TEXT NOT NULL UNIQUE,
                account_number TEXT,
                routing_number TEXT,
                wire_routing_number TEXT,
                verification_status TEXT,
                raw_auth_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create identities table for Plaid Identity data per account
        // Matches production Turso schema with JSON arrays and dedicated columns
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS identities (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                -- Plaid Identity data per account (JSON arrays for backward compatibility)
                account_holder_names TEXT, -- JSON array of names from Plaid Identity API
                account_holder_emails TEXT, -- JSON array of email objects from Plaid Identity API
                account_holder_phones TEXT, -- JSON array of phone objects from Plaid Identity API
                account_holder_addresses TEXT, -- JSON array of address objects from Plaid Identity API
                -- Dedicated email columns for different email types
                primary_email TEXT,
                secondary_email TEXT,
                other_email TEXT,
                -- Dedicated phone columns for different phone types
                primary_phone TEXT,
                home_phone TEXT,
                work_phone TEXT,
                mobile_phone TEXT,
                -- Dedicated address columns for primary address
                primary_address_street TEXT,
                primary_address_city TEXT,
                primary_address_region TEXT,
                primary_address_postal_code TEXT,
                primary_address_country TEXT,
                -- Dedicated address columns for secondary address
                secondary_address_street TEXT,
                secondary_address_city TEXT,
                secondary_address_region TEXT,
                secondary_address_postal_code TEXT,
                secondary_address_country TEXT,
                -- Account-specific identity flags
                is_primary_account_holder BOOLEAN DEFAULT FALSE,
                account_holder_type TEXT DEFAULT 'owner',
                raw_identity_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
            (),
        ).await?;

        // NOTE: credit_reports table removed - use 'reports' table instead (BlockID)

        // Create workflows table (matches Turso cloud unified schema)
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS workflows (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                workflow_type TEXT NOT NULL,
                workflow_name TEXT NOT NULL,
                workflow_description TEXT,
                workflow_status TEXT DEFAULT 'draft',
                workflow_data TEXT NOT NULL,
                trigger_type TEXT,
                trigger_config TEXT,
                is_active BOOLEAN DEFAULT FALSE,
                last_run_at DATETIME,
                next_run_at DATETIME,
                run_count INTEGER DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create indexes for workflows table
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_workflows_user_id ON workflows(user_id)",
                (),
            )
            .await?;

        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_workflows_updated ON workflows(updated_at DESC)",
                (),
            )
            .await?;

        // Note: sync_status index removed - column not in table definition

        // Create user_preferences table
        // P0g: Added onboarding dismissal fields for §27 onboarding flow rules
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS user_preferences (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL UNIQUE,
                ai_agent_enabled BOOLEAN DEFAULT FALSE,
                ai_feedback_enabled BOOLEAN DEFAULT FALSE,
                ai_offers_enabled BOOLEAN DEFAULT FALSE,
                ai_lenders_enabled BOOLEAN DEFAULT FALSE,
                cloud_sync_enabled BOOLEAN DEFAULT TRUE,
                blockchain_enabled BOOLEAN DEFAULT TRUE,
                email_notifications_enabled BOOLEAN DEFAULT TRUE,
                kilt_did_enabled BOOLEAN DEFAULT FALSE,
                ai_mode TEXT DEFAULT 'auto',
                mock_data_enabled BOOLEAN DEFAULT FALSE,
                -- P0g: Onboarding dismissal fields (§27.3)
                onboarding_completed BOOLEAN DEFAULT FALSE,
                onboarding_permanently_dismissed BOOLEAN DEFAULT FALSE,
                onboarding_reminder_dismissed_until DATETIME,
                plaid_connection_skipped BOOLEAN DEFAULT FALSE,
                plaid_reminder_dismissed_until DATETIME,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Add ai_mode column if it doesn't exist (migration for existing databases)
        let _ = self
            .connection
            .execute(
                "ALTER TABLE user_preferences ADD COLUMN ai_mode TEXT DEFAULT 'auto'",
                (),
            )
            .await;

        // Add mock_data_enabled column if it doesn't exist (migration for existing databases)
        // Only available for @freshcredit.com internal team members
        let _ = self
            .connection
            .execute(
                "ALTER TABLE user_preferences ADD COLUMN mock_data_enabled BOOLEAN DEFAULT FALSE",
                (),
            )
            .await;

        // P0g: Add onboarding dismissal columns (migration for existing databases)
        let _ = self
            .connection
            .execute(
                "ALTER TABLE user_preferences ADD COLUMN onboarding_completed BOOLEAN DEFAULT FALSE",
                (),
            )
            .await;
        let _ = self
            .connection
            .execute(
                "ALTER TABLE user_preferences ADD COLUMN onboarding_permanently_dismissed BOOLEAN DEFAULT FALSE",
                (),
            )
            .await;
        let _ = self
            .connection
            .execute(
                "ALTER TABLE user_preferences ADD COLUMN onboarding_reminder_dismissed_until DATETIME",
                (),
            )
            .await;
        let _ = self
            .connection
            .execute(
                "ALTER TABLE user_preferences ADD COLUMN plaid_connection_skipped BOOLEAN DEFAULT FALSE",
                (),
            )
            .await;
        let _ = self
            .connection
            .execute(
                "ALTER TABLE user_preferences ADD COLUMN plaid_reminder_dismissed_until DATETIME",
                (),
            )
            .await;

        // Create api_keys table for API key management
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                key_name TEXT NOT NULL,
                key_hash TEXT NOT NULL,
                key_prefix TEXT NOT NULL,
                permissions TEXT NOT NULL DEFAULT 'read',
                rate_limit INTEGER DEFAULT 1000,
                is_active BOOLEAN DEFAULT TRUE,
                last_used_at DATETIME,
                expires_at DATETIME,
                is_revoked BOOLEAN DEFAULT FALSE,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create index for api_keys lookup
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_api_keys_user_id ON api_keys(user_id)",
                (),
            )
            .await?;

        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_api_keys_key_prefix ON api_keys(key_prefix)",
                (),
            )
            .await?;

        // Create webauthn_credentials table for FIDO2/passkey biometric authentication
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS webauthn_credentials (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                credential_id BLOB NOT NULL UNIQUE,
                public_key BLOB NOT NULL,
                sign_count INTEGER NOT NULL DEFAULT 0,
                aaguid BLOB,
                credential_name TEXT,
                transports TEXT,
                attestation_format TEXT,
                user_verified BOOLEAN DEFAULT FALSE,
                backup_eligible BOOLEAN DEFAULT FALSE,
                backup_state BOOLEAN DEFAULT FALSE,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_used_at DATETIME,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_webauthn_credentials_user_id ON webauthn_credentials(user_id)",
                (),
            )
            .await?;

        self.connection
            .execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_webauthn_credentials_credential_id ON webauthn_credentials(credential_id)",
                (),
            )
            .await?;

        // Create kilt_dids table for KILT Protocol DID storage
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS kilt_dids (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                did_uri TEXT NOT NULL UNIQUE,
                network TEXT NOT NULL CHECK (network IN ('peregrine', 'spiritnet')),
                did_type TEXT NOT NULL CHECK (did_type IN ('light', 'full')),
                web3name TEXT,
                encrypted_keypair TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                (),
            )
            .await?;

        // Create index for kilt_dids lookup
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_kilt_dids_did_uri ON kilt_dids(did_uri)",
                (),
            )
            .await?;

        // Create uploaded_files table for AI multimodal input
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS uploaded_files (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                file_type TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                mime_type TEXT NOT NULL,
                storage_path TEXT,
                file_hash TEXT,
                file_data BLOB,
                text_content TEXT,
                ai_analysis TEXT,
                is_encrypted BOOLEAN DEFAULT FALSE,
                encryption_key_id TEXT,
                metadata TEXT,
                conversation_id TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                expires_at DATETIME,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create index for uploaded_files lookup
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_uploaded_files_user_id ON uploaded_files(user_id)",
                (),
            )
            .await?;

        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_uploaded_files_conversation ON uploaded_files(conversation_id)",
            (),
        ).await?;

        // Create ai_conversations table for conversation memory
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS ai_conversations (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                title TEXT,
                context TEXT,
                model TEXT DEFAULT 'gemini-pro',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create ai_messages table for conversation history
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS ai_messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                file_attachment_id TEXT,
                tokens_used INTEGER,
                model TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (conversation_id) REFERENCES ai_conversations(id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create indexes for ai_conversations
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_ai_conversations_user_id ON ai_conversations(user_id)",
            (),
        ).await?;

        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_ai_messages_conversation ON ai_messages(conversation_id)",
            (),
        ).await?;

        // ============================================
        // PLAID PRODUCT TABLES (from Turso production)
        // ============================================

        // Create assets table for Plaid Asset Reports
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS assets (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                asset_report_id TEXT NOT NULL UNIQUE,
                asset_report_token TEXT NOT NULL,
                client_report_id TEXT,
                date_generated DATETIME NOT NULL,
                days_requested INTEGER NOT NULL,
                report_type TEXT DEFAULT 'FULL',
                user_info TEXT,
                raw_asset_report_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create balances table for Plaid Balance data
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS balances (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                balance_current REAL NOT NULL,
                balance_available REAL,
                balance_limit REAL,
                iso_currency_code TEXT DEFAULT 'USD',
                unofficial_currency_code TEXT,
                last_statement_issue_date DATE,
                last_statement_balance REAL,
                minimum_balance_fee REAL,
                raw_balance_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create consumer_reports table for Plaid Consumer Reports
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS consumer_reports (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                consumer_report_id TEXT UNIQUE,
                report_type TEXT,
                permissible_purpose TEXT,
                report_status TEXT,
                report_generation_time DATETIME,
                report_expiration_time DATETIME,
                consumer_consent_given BOOLEAN DEFAULT FALSE,
                consumer_consent_timestamp DATETIME,
                credit_score INTEGER,
                credit_score_model TEXT,
                credit_score_factors TEXT,
                tradelines_count INTEGER DEFAULT 0,
                inquiries_count INTEGER DEFAULT 0,
                public_records_count INTEGER DEFAULT 0,
                collections_count INTEGER DEFAULT 0,
                report_data TEXT,
                raw_consumer_report_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create employment table for Plaid Employment data
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS employment (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                employment_id TEXT NOT NULL UNIQUE,
                employer_name TEXT,
                employer_address TEXT,
                employment_type TEXT,
                job_title TEXT,
                start_date DATE,
                end_date DATE,
                salary REAL,
                pay_frequency TEXT,
                currency TEXT DEFAULT 'USD',
                raw_employment_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create enrich table for Plaid Transaction Enrichment
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS enrich (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                transaction_id TEXT NOT NULL,
                enriched_merchant_name TEXT,
                enriched_category TEXT,
                enriched_subcategory TEXT,
                merchant_logo_url TEXT,
                merchant_website TEXT,
                merchant_phone_number TEXT,
                merchant_address TEXT,
                confidence_level REAL,
                enrichment_timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                raw_enrich_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE,
                FOREIGN KEY (transaction_id) REFERENCES transactions (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create income table for Plaid Bank Income
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS income (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                bank_income_id TEXT NOT NULL UNIQUE,
                generated_time DATETIME NOT NULL,
                days_requested INTEGER NOT NULL,
                item_id TEXT NOT NULL,
                institution_id TEXT,
                institution_name TEXT,
                raw_bank_income_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create income_verification table for Plaid Income Verification
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS income_verification (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                income_verification_id TEXT UNIQUE,
                user_token TEXT,
                webhook_url TEXT,
                status TEXT,
                created_at_plaid DATETIME,
                completed_at DATETIME,
                days_requested INTEGER DEFAULT 365,
                transactions_access_token TEXT,
                transactions_access_tokens TEXT,
                income_source_types TEXT,
                income_verification_url TEXT,
                income_report_token TEXT,
                precheck_id TEXT,
                raw_income_verification_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create identity_verification table for Plaid IDV (singular)
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS identity_verification (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                identity_verification_id TEXT UNIQUE,
                template_id TEXT,
                gave_consent BOOLEAN DEFAULT FALSE,
                status TEXT,
                created_at_plaid DATETIME,
                completed_at DATETIME,
                previous_attempt_id TEXT,
                shareable_url TEXT,
                client_user_id TEXT,
                phone_number TEXT,
                email_address TEXT,
                date_of_birth DATE,
                country_code TEXT,
                documentary_verification TEXT,
                selfie_verification TEXT,
                kyc_check TEXT,
                risk_check TEXT,
                watchlist_screening TEXT,
                raw_identity_verification_data TEXT NOT NULL,
                blockchain_hash TEXT,
                block_number INTEGER,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // NOTE: identity_verifications (plural) table removed - use 'identity_verification' (singular) instead

        // Create investments_holdings table for Plaid Investments
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS investments_holdings (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                security_id TEXT NOT NULL,
                security_type TEXT,
                asset_class TEXT,
                institution_price REAL,
                institution_price_as_of DATE,
                institution_price_datetime DATETIME,
                institution_value REAL,
                cost_basis REAL,
                quantity REAL NOT NULL,
                iso_currency_code TEXT DEFAULT 'USD',
                unofficial_currency_code TEXT,
                vested_quantity REAL,
                vested_value REAL,
                raw_holding_data TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create investments_securities table for Plaid Investments
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS investments_securities (
                id TEXT PRIMARY KEY,
                security_id TEXT NOT NULL UNIQUE,
                isin TEXT,
                cusip TEXT,
                sedol TEXT,
                institution_security_id TEXT,
                institution_id TEXT,
                proxy_security_id TEXT,
                name TEXT,
                ticker_symbol TEXT,
                is_cash_equivalent BOOLEAN DEFAULT FALSE,
                type TEXT,
                close_price REAL,
                close_price_as_of DATE,
                update_datetime DATETIME,
                iso_currency_code TEXT DEFAULT 'USD',
                unofficial_currency_code TEXT,
                market_identifier_code TEXT,
                sector TEXT,
                industry TEXT,
                option_contract TEXT,
                raw_security_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                (),
            )
            .await?;

        // Create investments_transactions table for Plaid Investments
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS investments_transactions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                security_id TEXT,
                investment_transaction_id TEXT NOT NULL UNIQUE,
                date DATE NOT NULL,
                name TEXT NOT NULL,
                quantity REAL NOT NULL,
                amount REAL NOT NULL,
                price REAL NOT NULL,
                fees REAL,
                type TEXT NOT NULL,
                subtype TEXT,
                iso_currency_code TEXT DEFAULT 'USD',
                unofficial_currency_code TEXT,
                raw_investment_transaction_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create layer table for Plaid Layer
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS layer (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                layer_id TEXT NOT NULL UNIQUE,
                layer_type TEXT,
                layer_status TEXT,
                layer_data TEXT,
                raw_layer_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create liabilities table for Plaid Liabilities
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS liabilities (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                liability_type TEXT NOT NULL,
                -- Credit card specific fields
                aprs TEXT,
                is_overdue BOOLEAN,
                last_payment_amount REAL,
                last_payment_date DATE,
                last_statement_issue_date DATE,
                last_statement_balance REAL,
                minimum_payment_amount REAL,
                next_payment_due_date DATE,
                -- Mortgage specific fields
                origination_date DATE,
                origination_principal_amount REAL,
                current_late_fee REAL,
                escrow_balance REAL,
                has_pmi BOOLEAN,
                has_prepayment_penalty BOOLEAN,
                interest_rate_percentage REAL,
                interest_rate_type TEXT,
                loan_term TEXT,
                loan_type_description TEXT,
                maturity_date DATE,
                next_monthly_payment REAL,
                past_due_amount REAL,
                property_address TEXT,
                ytd_interest_paid REAL,
                ytd_principal_paid REAL,
                -- Student loan specific fields
                disbursement_dates TEXT,
                expected_payoff_date DATE,
                guarantor TEXT,
                interest_rate_percentage_student REAL,
                is_overdue_student BOOLEAN,
                last_payment_amount_student REAL,
                last_payment_date_student DATE,
                last_statement_issue_date_student DATE,
                last_statement_balance_student REAL,
                loan_name TEXT,
                loan_status TEXT,
                minimum_payment_amount_student REAL,
                next_payment_due_date_student DATE,
                origination_date_student DATE,
                origination_principal_amount_student REAL,
                outstanding_interest_amount REAL,
                payment_reference_number TEXT,
                pslf_status TEXT,
                repayment_plan TEXT,
                sequence_number TEXT,
                servicer_address TEXT,
                ytd_interest_paid_student REAL,
                ytd_principal_paid_student REAL,
                raw_liability_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create monitor table for Plaid Monitor (Item monitoring)
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS monitor (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                item_id TEXT NOT NULL,
                monitor_id TEXT NOT NULL UNIQUE,
                monitor_type TEXT,
                monitor_status TEXT,
                last_check_time DATETIME,
                next_check_time DATETIME,
                alert_settings TEXT,
                raw_monitor_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create recurring_transactions table for Plaid Recurring Transactions
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS recurring_transactions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                stream_id TEXT NOT NULL UNIQUE,
                category TEXT,
                category_id TEXT,
                description TEXT,
                merchant_name TEXT,
                personal_finance_category TEXT,
                first_date DATE,
                last_date DATE,
                frequency TEXT,
                transaction_ids TEXT,
                average_amount REAL,
                average_amount_is_estimated BOOLEAN DEFAULT FALSE,
                last_amount REAL,
                is_active BOOLEAN DEFAULT TRUE,
                status TEXT,
                is_user_modified BOOLEAN DEFAULT FALSE,
                last_user_modified_datetime DATETIME,
                raw_recurring_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create signal_evaluations table for Plaid Signal
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS signal_evaluations (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                client_transaction_id TEXT NOT NULL UNIQUE,
                amount REAL NOT NULL,
                client_user_id TEXT,
                user_present BOOLEAN,
                device TEXT,
                scores TEXT,
                core_attributes TEXT,
                warnings TEXT,
                request_id TEXT,
                raw_signal_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create statements table for Plaid Statements
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS statements (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                statement_id TEXT NOT NULL UNIQUE,
                month INTEGER NOT NULL,
                year INTEGER NOT NULL,
                institution_id TEXT,
                institution_name TEXT,
                statement_pdf_url TEXT,
                statement_pdf_data BLOB,
                raw_statement_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create transactions_sync table for Plaid Transactions Sync cursor
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS transactions_sync (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                item_id TEXT NOT NULL UNIQUE,
                access_token TEXT NOT NULL,
                cursor TEXT,
                has_more BOOLEAN DEFAULT FALSE,
                added_count INTEGER DEFAULT 0,
                modified_count INTEGER DEFAULT 0,
                removed_count INTEGER DEFAULT 0,
                last_sync_time DATETIME,
                raw_sync_data TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // ============================================
        // PAYMENT AND BUSINESS TABLES
        // ============================================

        // Create customers table for Stripe/Dwolla customers
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS customers (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                stripe_customer_id TEXT UNIQUE,
                dwolla_customer_id TEXT UNIQUE,
                customer_type TEXT DEFAULT 'consumer',
                email TEXT,
                phone TEXT,
                first_name TEXT,
                last_name TEXT,
                business_name TEXT,
                business_type TEXT,
                status TEXT DEFAULT 'active',
                verification_status TEXT,
                raw_customer_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create funding_sources table for payment funding sources
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS funding_sources (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                customer_id TEXT NOT NULL,
                account_id TEXT,
                funding_source_id TEXT UNIQUE,
                funding_source_type TEXT NOT NULL,
                bank_name TEXT,
                bank_account_type TEXT,
                name TEXT,
                status TEXT DEFAULT 'unverified',
                verification_type TEXT,
                is_default BOOLEAN DEFAULT FALSE,
                raw_funding_source_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE SET NULL
            )",
                (),
            )
            .await?;

        // Create payments table for payment transactions
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS payments (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                customer_id TEXT NOT NULL,
                funding_source_id TEXT,
                payment_id TEXT UNIQUE,
                payment_type TEXT NOT NULL,
                amount REAL NOT NULL,
                currency TEXT DEFAULT 'USD',
                status TEXT DEFAULT 'pending',
                description TEXT,
                metadata TEXT,
                failure_reason TEXT,
                initiated_at DATETIME,
                completed_at DATETIME,
                raw_payment_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE,
                FOREIGN KEY (funding_source_id) REFERENCES funding_sources (id) ON DELETE SET NULL
            )",
                (),
            )
            .await?;

        // Create stripe_plaid_payments table for Stripe+Plaid ACH payments
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS stripe_plaid_payments (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                stripe_payment_intent_id TEXT UNIQUE,
                plaid_account_id TEXT NOT NULL,
                amount REAL NOT NULL,
                currency TEXT DEFAULT 'USD',
                status TEXT DEFAULT 'pending',
                stripe_customer_id TEXT,
                stripe_payment_method_id TEXT,
                plaid_access_token TEXT,
                description TEXT,
                metadata TEXT,
                failure_reason TEXT,
                initiated_at DATETIME,
                completed_at DATETIME,
                raw_payment_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create virtual_accounts table for virtual account numbers
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS virtual_accounts (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                customer_id TEXT NOT NULL,
                virtual_account_id TEXT UNIQUE,
                account_number TEXT,
                routing_number TEXT,
                account_type TEXT DEFAULT 'checking',
                status TEXT DEFAULT 'active',
                balance REAL DEFAULT 0.0,
                currency TEXT DEFAULT 'USD',
                raw_virtual_account_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // ============================================
        // PROVIDER AND BUSINESS LOGIC TABLES
        // ============================================

        // Create reports table for generated reports (BlockID)
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS reports (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                report_type TEXT NOT NULL,
                report_name TEXT,
                report_status TEXT DEFAULT 'pending',
                generation_started_at DATETIME,
                generation_completed_at DATETIME,
                expiration_date DATETIME,
                blockchain_hash TEXT,
                blockchain_tx_id TEXT,
                report_data TEXT,
                raw_report_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create scores table for provider-defined scoring models (BlockScore)
        // NOTE: FreshCredit does NOT generate scores - providers define their own models
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS scores (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                score_model_id TEXT NOT NULL,
                score_model_name TEXT,
                score_model_version TEXT,
                -- Provider-defined score (FreshCredit does not calculate this)
                provider_calculated_score INTEGER,
                provider_score_factors TEXT,
                -- Data elements used for scoring (weights defined by provider)
                data_elements_used TEXT,
                data_element_weights TEXT,
                -- Metadata
                calculated_at DATETIME,
                expires_at DATETIME,
                raw_score_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create offers table for matched offers (BlockIQ)
        // NOTE: FreshCredit matches offers, does not recommend them
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS offers (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                offer_type TEXT NOT NULL,
                offer_name TEXT,
                offer_description TEXT,
                -- Match criteria (not recommendation)
                match_score REAL,
                match_criteria TEXT,
                user_preferences_matched TEXT,
                provider_requirements_matched TEXT,
                -- Offer details (provider-defined)
                offer_terms TEXT,
                offer_amount_min REAL,
                offer_amount_max REAL,
                offer_apr_min REAL,
                offer_apr_max REAL,
                offer_duration_months INTEGER,
                -- Status
                offer_status TEXT DEFAULT 'active',
                user_viewed_at DATETIME,
                user_clicked_at DATETIME,
                user_applied_at DATETIME,
                expires_at DATETIME,
                raw_offer_data TEXT NOT NULL,
                blockchain_hash TEXT,
                block_number INTEGER,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create provider_offers table for provider product catalog (BlockIQ)
        // COMPLIANCE: §2 - Neutral matching only, no recommendations
        // NOTE: Score thresholds are NOT stored here - they belong in workflow Decision nodes
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS provider_offers (
                id TEXT PRIMARY KEY,
                provider_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                product_type TEXT NOT NULL,
                -- Product terms (what provider offers)
                loan_amount_min_cents INTEGER,
                loan_amount_max_cents INTEGER,
                apr_min_percent REAL,
                apr_max_percent REAL,
                term_options_months TEXT,
                fees_json TEXT,
                rewards_json TEXT,
                -- Geographic targeting
                included_states TEXT,
                excluded_states TEXT,
                included_zip_codes TEXT,
                nationwide BOOLEAN DEFAULT TRUE,
                -- Customer segment targeting
                customer_segment TEXT DEFAULT 'all',
                customer_profiles TEXT,
                -- BlockScore model reference (for requirements matching)
                blockscore_model_id TEXT,
                -- A/B testing support
                ab_test_variant TEXT,
                ab_test_allocation INTEGER,
                -- Metadata
                is_active BOOLEAN DEFAULT TRUE,
                version INTEGER DEFAULT 1,
                blockchain_hash TEXT,
                block_number INTEGER,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (provider_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create disputes table for consumer disputes
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS disputes (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                dispute_type TEXT NOT NULL,
                disputed_item_type TEXT,
                disputed_item_id TEXT,
                dispute_reason TEXT NOT NULL,
                dispute_description TEXT,
                supporting_documents TEXT,
                dispute_status TEXT DEFAULT 'pending',
                resolution TEXT,
                resolved_at DATETIME,
                raw_dispute_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create verification_requests table for data verification requests
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS verification_requests (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                request_type TEXT NOT NULL,
                requested_data_elements TEXT,
                consent_given BOOLEAN DEFAULT FALSE,
                consent_timestamp DATETIME,
                consent_expires_at DATETIME,
                request_status TEXT DEFAULT 'pending',
                verification_result TEXT,
                verified_at DATETIME,
                raw_request_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create phone_verification_codes table for SMS verification
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS phone_verification_codes (
                id TEXT PRIMARY KEY,
                user_id TEXT,
                phone_number TEXT NOT NULL,
                code TEXT NOT NULL,
                purpose TEXT NOT NULL DEFAULT 'phone_verification',
                attempts INTEGER DEFAULT 0,
                max_attempts INTEGER DEFAULT 3,
                expires_at DATETIME NOT NULL,
                verified_at DATETIME,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                (),
            )
            .await?;

        // Create verified_credentials table for Entra Verified ID / KILT DID credentials
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS verified_credentials (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                credential_type TEXT NOT NULL,
                credential_issuer TEXT NOT NULL,
                credential_subject TEXT,
                credential_id TEXT UNIQUE,
                did_uri TEXT,
                issuance_date DATETIME,
                expiration_date DATETIME,
                credential_status TEXT DEFAULT 'active',
                revocation_id TEXT,
                credential_data TEXT,
                raw_credential_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // ============================================
        // WEBHOOK EVENTS TABLE (Outbox Pattern)
        // ============================================
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS webhook_events (
                id TEXT PRIMARY KEY,
                user_id TEXT,
                provider TEXT NOT NULL,
                event_type TEXT NOT NULL,
                event_id TEXT UNIQUE,
                payload TEXT NOT NULL,
                status TEXT DEFAULT 'pending',
                retry_count INTEGER DEFAULT 0,
                error_message TEXT,
                processed_at DATETIME,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE SET NULL
            )",
                (),
            )
            .await?;

        // ============================================
        // NOTIFICATIONS TABLE (In-App Notifications)
        // ============================================
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS notifications (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                notification_type TEXT NOT NULL,
                title TEXT NOT NULL,
                message TEXT,
                read_status INTEGER DEFAULT 0,
                action_url TEXT,
                metadata TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                expires_at DATETIME,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // ============================================
        // TICKETING SYSTEM TABLES
        // ============================================

        // Tickets table (Support tickets, disputes, inquiries)
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS tickets (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                ticket_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                description TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'open',
                priority TEXT NOT NULL DEFAULT 'medium',
                category TEXT,
                related_entity_type TEXT,
                related_entity_id TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                resolved_at DATETIME,
                sla_due_at DATETIME,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Ticket comments table
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS ticket_comments (
                id TEXT PRIMARY KEY,
                ticket_id TEXT NOT NULL,
                author_id TEXT NOT NULL,
                content TEXT NOT NULL,
                is_internal INTEGER DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (ticket_id) REFERENCES tickets (id) ON DELETE CASCADE,
                FOREIGN KEY (author_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Ticket assignments table
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS ticket_assignments (
                id TEXT PRIMARY KEY,
                ticket_id TEXT NOT NULL,
                assignee_id TEXT NOT NULL,
                assigned_by TEXT NOT NULL,
                assigned_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                unassigned_at DATETIME,
                FOREIGN KEY (ticket_id) REFERENCES tickets (id) ON DELETE CASCADE,
                FOREIGN KEY (assignee_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (assigned_by) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Ticket SLA events table
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS ticket_sla_events (
                id TEXT PRIMARY KEY,
                ticket_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                occurred_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                sla_target_minutes INTEGER,
                actual_minutes INTEGER,
                FOREIGN KEY (ticket_id) REFERENCES tickets (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // ============================================
        // COMPLIANCE MONITORING TABLES
        // ============================================

        // Compliance scans table
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS compliance_scans (
                id TEXT PRIMARY KEY,
                scan_type TEXT NOT NULL,
                framework TEXT,
                status TEXT NOT NULL DEFAULT 'running',
                started_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                completed_at DATETIME,
                findings_count INTEGER DEFAULT 0,
                triggered_by TEXT
            )",
                (),
            )
            .await?;

        // Compliance rules table
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS compliance_rules (
                id TEXT PRIMARY KEY,
                framework TEXT NOT NULL,
                rule_code TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                severity TEXT NOT NULL,
                detection_pattern TEXT,
                remediation_template TEXT,
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                (),
            )
            .await?;

        // Compliance findings table
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS compliance_findings (
                id TEXT PRIMARY KEY,
                scan_id TEXT NOT NULL,
                rule_id TEXT NOT NULL,
                severity TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'open',
                file_path TEXT,
                line_number INTEGER,
                description TEXT NOT NULL,
                remediation_guidance TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                resolved_at DATETIME,
                resolved_by TEXT,
                FOREIGN KEY (scan_id) REFERENCES compliance_scans (id) ON DELETE CASCADE,
                FOREIGN KEY (rule_id) REFERENCES compliance_rules (id) ON DELETE CASCADE,
                FOREIGN KEY (resolved_by) REFERENCES user_profile (id) ON DELETE SET NULL
            )",
                (),
            )
            .await?;

        // Compliance evidence table
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS compliance_evidence (
                id TEXT PRIMARY KEY,
                finding_id TEXT NOT NULL,
                evidence_type TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (finding_id) REFERENCES compliance_findings (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // ============================================
        // INDEXES FOR ALL TABLES
        // ============================================

        // Indexes for Plaid product tables
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_assets_user_id ON assets(user_id)",
                (),
            )
            .await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_balances_account_id ON balances(account_id)",
                (),
            )
            .await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_consumer_reports_user_id ON consumer_reports(user_id)",
            (),
        ).await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_employment_user_id ON employment(user_id)",
                (),
            )
            .await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_enrich_transaction_id ON enrich(transaction_id)",
                (),
            )
            .await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_income_user_id ON income(user_id)",
                (),
            )
            .await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_income_verification_user_id ON income_verification(user_id)",
            (),
        ).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_identity_verification_user_id ON identity_verification(user_id)",
            (),
        ).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_investments_holdings_account_id ON investments_holdings(account_id)",
            (),
        ).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_investments_securities_ticker ON investments_securities(ticker_symbol)",
            (),
        ).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_investments_transactions_account_id ON investments_transactions(account_id)",
            (),
        ).await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_layer_account_id ON layer(account_id)",
                (),
            )
            .await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_liabilities_account_id ON liabilities(account_id)",
                (),
            )
            .await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_monitor_item_id ON monitor(item_id)",
                (),
            )
            .await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_recurring_transactions_account_id ON recurring_transactions(account_id)",
            (),
        ).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_signal_evaluations_account_id ON signal_evaluations(account_id)",
            (),
        ).await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_statements_account_id ON statements(account_id)",
                (),
            )
            .await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_transactions_sync_item_id ON transactions_sync(item_id)",
            (),
        ).await?;

        // Indexes for payment tables
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_customers_user_id ON customers(user_id)",
                (),
            )
            .await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_funding_sources_customer_id ON funding_sources(customer_id)",
            (),
        ).await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_payments_customer_id ON payments(customer_id)",
                (),
            )
            .await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_stripe_plaid_payments_user_id ON stripe_plaid_payments(user_id)",
            (),
        ).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_virtual_accounts_customer_id ON virtual_accounts(customer_id)",
            (),
        ).await?;

        // Indexes for business logic tables
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_reports_user_id ON reports(user_id)",
                (),
            )
            .await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_scores_user_id ON scores(user_id)",
                (),
            )
            .await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_scores_provider_id ON scores(provider_id)",
                (),
            )
            .await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_offers_user_id ON offers(user_id)",
                (),
            )
            .await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_offers_provider_id ON offers(provider_id)",
                (),
            )
            .await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_disputes_user_id ON disputes(user_id)",
                (),
            )
            .await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_verification_requests_user_id ON verification_requests(user_id)",
            (),
        ).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_phone_verification_codes_phone ON phone_verification_codes(phone_number)",
            (),
        ).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_phone_verification_codes_expires ON phone_verification_codes(expires_at)",
            (),
        ).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_verified_credentials_user_id ON verified_credentials(user_id)",
            (),
        ).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_verified_credentials_did_uri ON verified_credentials(did_uri)",
            (),
        ).await?;

        // Indexes for identities table (new comprehensive schema)
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_identities_account_id ON identities(account_id)",
                (),
            )
            .await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_identities_user_id ON identities(user_id)",
                (),
            )
            .await?;

        // Indexes for webhook_events table
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_webhook_events_user_id ON webhook_events(user_id)",
                (),
            )
            .await?;
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_webhook_events_status ON webhook_events(status)",
                (),
            )
            .await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_webhook_events_provider ON webhook_events(provider)",
            (),
        ).await?;

        // Indexes for notifications table
        self.connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_notifications_user_id ON notifications(user_id)",
                (),
            )
            .await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_notifications_read_status ON notifications(read_status)",
            (),
        ).await?;

        // Indexes for ticketing system tables
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_tickets_user_id ON tickets(user_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_tickets_status ON tickets(status)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_tickets_priority ON tickets(priority)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_tickets_type ON tickets(ticket_type)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_tickets_created_at ON tickets(created_at)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_ticket_comments_ticket_id ON ticket_comments(ticket_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_ticket_comments_author_id ON ticket_comments(author_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_ticket_assignments_ticket_id ON ticket_assignments(ticket_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_ticket_assignments_assignee_id ON ticket_assignments(assignee_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_ticket_sla_events_ticket_id ON ticket_sla_events(ticket_id)", ()).await?;

        // Indexes for compliance monitoring tables
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_compliance_scans_status ON compliance_scans(status)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_compliance_scans_framework ON compliance_scans(framework)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_compliance_scans_started_at ON compliance_scans(started_at)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_compliance_rules_framework ON compliance_rules(framework)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_compliance_rules_severity ON compliance_rules(severity)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_compliance_rules_is_active ON compliance_rules(is_active)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_compliance_findings_scan_id ON compliance_findings(scan_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_compliance_findings_rule_id ON compliance_findings(rule_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_compliance_findings_severity ON compliance_findings(severity)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_compliance_findings_status ON compliance_findings(status)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_compliance_evidence_finding_id ON compliance_evidence(finding_id)", ()).await?;

        // ============================================
        // DATA APPROVAL HASHES TABLE
        // ============================================
        // Stores blockchain hashes generated when users approve their Plaid data
        // Stage 1 of two-stage blockchain hashing: hash at approval time
        // Stage 2 is at report generation time (stored in reports table)
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS data_approval_hashes (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                approval_type TEXT NOT NULL DEFAULT 'plaid_data',
                data_hash TEXT NOT NULL,
                blockchain_hash TEXT,
                blockchain_tx_id TEXT,
                blockchain_block_number INTEGER,
                item_count INTEGER NOT NULL DEFAULT 0,
                accounts_count INTEGER DEFAULT 0,
                transactions_count INTEGER DEFAULT 0,
                data_summary TEXT,
                approval_status TEXT NOT NULL DEFAULT 'pending',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                anchored_at DATETIME,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create indexes for data_approval_hashes
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_data_approval_hashes_user_id ON data_approval_hashes(user_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_data_approval_hashes_blockchain_hash ON data_approval_hashes(blockchain_hash)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_data_approval_hashes_created_at ON data_approval_hashes(created_at)", ()).await?;

        // ============================================================================
        // SECTION 16: REFERRALS TABLE
        // ============================================================================
        // Stores referral tracking for rewards program
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS referrals (
                id TEXT PRIMARY KEY,
                referrer_user_id TEXT NOT NULL,
                referred_user_id TEXT,
                referral_code TEXT NOT NULL,
                status TEXT DEFAULT 'pending',
                earnings_cents INTEGER DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                converted_at DATETIME,
                FOREIGN KEY (referrer_user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create indexes for referrals
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_referrals_referrer_user_id ON referrals(referrer_user_id)", ()).await?;
        self.connection.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_referrals_code ON referrals(referral_code)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_referrals_status ON referrals(status)", ()).await?;

        // ============================================================================
        // SECTION 17: PLATFORM METRICS TABLE
        // ============================================================================
        // Stores daily aggregated platform metrics for internal dashboard
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS platform_metrics (
                id TEXT PRIMARY KEY,
                metric_date DATE NOT NULL,
                total_users INTEGER DEFAULT 0,
                new_users INTEGER DEFAULT 0,
                active_providers INTEGER DEFAULT 0,
                pending_providers INTEGER DEFAULT 0,
                reports_generated INTEGER DEFAULT 0,
                reports_verified INTEGER DEFAULT 0,
                revenue_cents INTEGER DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                (),
            )
            .await?;

        // Create indexes for platform_metrics
        self.connection.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_platform_metrics_date ON platform_metrics(metric_date)", ()).await?;

        // ============================================================================
        // SECTION 18: SALES PIPELINE TABLE
        // ============================================================================
        // Stores sales pipeline data synced from HubSpot
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS sales_pipeline (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                hubspot_deal_id TEXT UNIQUE,
                deal_name TEXT,
                stage TEXT NOT NULL,
                value_cents INTEGER,
                contact_email TEXT,
                company_name TEXT,
                last_activity_at DATETIME,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                closed_at DATETIME,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create indexes for sales_pipeline
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_sales_pipeline_user_id ON sales_pipeline(user_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_sales_pipeline_stage ON sales_pipeline(stage)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_sales_pipeline_hubspot_deal_id ON sales_pipeline(hubspot_deal_id)", ()).await?;

        // ============================================================
        // LinkedIn Professional Data Tables (L1: Multi-Source Integration)
        // ============================================================

        // Create linkedin_profiles table for LinkedIn profile data
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS linkedin_profiles (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                linkedin_id TEXT UNIQUE,
                public_profile_url TEXT,
                first_name TEXT,
                last_name TEXT,
                headline TEXT,
                summary TEXT,
                industry TEXT,
                location TEXT,
                country_code TEXT,
                profile_picture_url TEXT,
                connections_count INTEGER,
                raw_profile_data TEXT NOT NULL,
                imported_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create linkedin_experiences table for work experience
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS linkedin_experiences (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                linkedin_profile_id TEXT NOT NULL,
                company_name TEXT NOT NULL,
                company_linkedin_url TEXT,
                title TEXT NOT NULL,
                description TEXT,
                location TEXT,
                employment_type TEXT,
                start_date TEXT,
                end_date TEXT,
                is_current BOOLEAN DEFAULT FALSE,
                raw_experience_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (linkedin_profile_id) REFERENCES linkedin_profiles (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create linkedin_education table for education history
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS linkedin_education (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                linkedin_profile_id TEXT NOT NULL,
                school_name TEXT NOT NULL,
                school_linkedin_url TEXT,
                degree TEXT,
                field_of_study TEXT,
                description TEXT,
                activities TEXT,
                start_date TEXT,
                end_date TEXT,
                grade TEXT,
                raw_education_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (linkedin_profile_id) REFERENCES linkedin_profiles (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create linkedin_skills table for skills and endorsements
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS linkedin_skills (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                linkedin_profile_id TEXT NOT NULL,
                skill_name TEXT NOT NULL,
                endorsement_count INTEGER DEFAULT 0,
                raw_skill_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (linkedin_profile_id) REFERENCES linkedin_profiles (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create linkedin_certifications table for professional certifications
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS linkedin_certifications (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                linkedin_profile_id TEXT NOT NULL,
                certification_name TEXT NOT NULL,
                issuing_organization TEXT,
                issue_date TEXT,
                expiration_date TEXT,
                credential_id TEXT,
                credential_url TEXT,
                raw_certification_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (linkedin_profile_id) REFERENCES linkedin_profiles (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create linkedin_languages table for language proficiencies
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS linkedin_languages (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                linkedin_profile_id TEXT NOT NULL,
                language_name TEXT NOT NULL,
                proficiency TEXT,
                raw_language_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (linkedin_profile_id) REFERENCES linkedin_profiles (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create indexes for LinkedIn tables
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_linkedin_profiles_user_id ON linkedin_profiles(user_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_linkedin_experiences_profile ON linkedin_experiences(linkedin_profile_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_linkedin_education_profile ON linkedin_education(linkedin_profile_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_linkedin_skills_profile ON linkedin_skills(linkedin_profile_id)", ()).await?;

        // =====================================================================
        // Section 20: HealthKit Tables (Apple Health data from XML export)
        // =====================================================================

        // Create healthkit_profiles table for user health profile metadata
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS healthkit_profiles (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL UNIQUE,
                export_date DATETIME,
                date_of_birth TEXT,
                biological_sex TEXT,
                blood_type TEXT,
                fitzpatrick_skin_type TEXT,
                wheelchair_use TEXT,
                raw_profile_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create healthkit_records table for health records (steps, heart rate, etc.)
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS healthkit_records (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                healthkit_profile_id TEXT NOT NULL,
                record_type TEXT NOT NULL,
                source_name TEXT,
                source_version TEXT,
                device TEXT,
                unit TEXT,
                value REAL,
                start_date DATETIME NOT NULL,
                end_date DATETIME NOT NULL,
                creation_date DATETIME,
                metadata TEXT,
                raw_record_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (healthkit_profile_id) REFERENCES healthkit_profiles (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create healthkit_workouts table for workout sessions
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS healthkit_workouts (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                healthkit_profile_id TEXT NOT NULL,
                workout_activity_type TEXT NOT NULL,
                duration REAL,
                duration_unit TEXT,
                total_distance REAL,
                distance_unit TEXT,
                total_energy_burned REAL,
                energy_unit TEXT,
                source_name TEXT,
                source_version TEXT,
                device TEXT,
                start_date DATETIME NOT NULL,
                end_date DATETIME NOT NULL,
                creation_date DATETIME,
                metadata TEXT,
                raw_workout_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (healthkit_profile_id) REFERENCES healthkit_profiles (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create healthkit_activity_summaries table for daily activity rings
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS healthkit_activity_summaries (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                healthkit_profile_id TEXT NOT NULL,
                date_components TEXT NOT NULL,
                active_energy_burned REAL,
                active_energy_burned_goal REAL,
                active_energy_burned_unit TEXT,
                apple_move_time REAL,
                apple_move_time_goal REAL,
                apple_exercise_time REAL,
                apple_exercise_time_goal REAL,
                apple_stand_hours REAL,
                apple_stand_hours_goal REAL,
                raw_summary_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (healthkit_profile_id) REFERENCES healthkit_profiles (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create healthkit_clinical_records table for clinical health records
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS healthkit_clinical_records (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                healthkit_profile_id TEXT NOT NULL,
                clinical_type TEXT NOT NULL,
                identifier TEXT,
                source_name TEXT,
                source_url TEXT,
                fhir_resource_type TEXT,
                fhir_resource_data TEXT,
                start_date DATETIME,
                end_date DATETIME,
                raw_clinical_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (healthkit_profile_id) REFERENCES healthkit_profiles (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create healthkit_correlations table for correlated health data
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS healthkit_correlations (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                healthkit_profile_id TEXT NOT NULL,
                correlation_type TEXT NOT NULL,
                source_name TEXT,
                start_date DATETIME NOT NULL,
                end_date DATETIME NOT NULL,
                objects TEXT,
                metadata TEXT,
                raw_correlation_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (healthkit_profile_id) REFERENCES healthkit_profiles (id) ON DELETE CASCADE
            )",
                (),
            )
            .await?;

        // Create indexes for HealthKit tables
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_healthkit_profiles_user_id ON healthkit_profiles(user_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_healthkit_records_profile ON healthkit_records(healthkit_profile_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_healthkit_records_type ON healthkit_records(record_type)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_healthkit_records_date ON healthkit_records(start_date)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_healthkit_workouts_profile ON healthkit_workouts(healthkit_profile_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_healthkit_activity_profile ON healthkit_activity_summaries(healthkit_profile_id)", ()).await?;

        // =====================================================================
        // Section 21: Correlation Engine Tables (Cross-source data correlation)
        // =====================================================================

        // Create correlation_preferences table for user correlation settings
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS correlation_preferences (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES user_profile(id),
                    source_type TEXT NOT NULL,
                    enabled INTEGER NOT NULL DEFAULT 1,
                    anonymization_level TEXT NOT NULL DEFAULT 'aggregate',
                    retention_days INTEGER NOT NULL DEFAULT 90,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE(user_id, source_type)
                )",
                (),
            )
            .await?;

        // Create correlation_insights table for generated insights
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS correlation_insights (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES user_profile(id),
                    insight_type TEXT NOT NULL,
                    title TEXT NOT NULL,
                    description TEXT NOT NULL,
                    sources TEXT NOT NULL,
                    confidence_score REAL NOT NULL,
                    data_points INTEGER NOT NULL,
                    insight_data TEXT,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    expires_at TEXT
                )",
                (),
            )
            .await?;

        // Create correlation_metrics table for aggregated metrics
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS correlation_metrics (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES user_profile(id),
                    metric_type TEXT NOT NULL,
                    metric_name TEXT NOT NULL,
                    metric_value REAL NOT NULL,
                    period_start TEXT NOT NULL,
                    period_end TEXT NOT NULL,
                    sources TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                )",
                (),
            )
            .await?;

        // Create indexes for correlation tables
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_correlation_prefs_user ON correlation_preferences(user_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_correlation_insights_user ON correlation_insights(user_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_correlation_insights_type ON correlation_insights(insight_type)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_correlation_metrics_user ON correlation_metrics(user_id)", ()).await?;
        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_correlation_metrics_type ON correlation_metrics(metric_type)", ()).await?;

        // HARDCODED_SCHEMA: 73 tables - update if schema changes (58 + 6 LinkedIn + 6 HealthKit + 3 Correlation)
        info!("Unified database schema initialization completed (73 tables)");
        Ok(())
    }

    /// Validate database schema integrity
    pub async fn validate_schema_integrity(&self) -> Result<SchemaValidationResult> {
        info!("Validating database schema integrity");

        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        // Check if foreign key constraints are enabled
        let mut rows = self.connection.query("PRAGMA foreign_keys", ()).await?;
        if let Some(row) = rows.next().await? {
            let fk_enabled: i64 = row.get(0)?;
            if fk_enabled == 0 {
                issues.push("Foreign key constraints are not enabled".to_string());
            }
        }

        // Check for orphaned transactions (transactions without valid accounts)
        let mut rows = self
            .connection
            .query(
                "SELECT COUNT(*) FROM transactions t
             LEFT JOIN accounts a ON t.account_id = a.id
             WHERE a.id IS NULL",
                (),
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let orphaned_count: i64 = row.get(0)?;
            if orphaned_count > 0 {
                issues.push(format!(
                    "Found {orphaned_count} orphaned transactions without valid accounts"
                ));
            }
        }

        // Check for accounts without valid users
        // Note: Table is user_profile (singular), not user_profiles
        let mut rows = self
            .connection
            .query(
                "SELECT COUNT(*) FROM accounts a
             LEFT JOIN user_profile u ON a.user_id = u.platform_user_id
             WHERE u.platform_user_id IS NULL",
                (),
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let orphaned_count: i64 = row.get(0)?;
            if orphaned_count > 0 {
                issues.push(format!(
                    "Found {orphaned_count} accounts without valid user profiles"
                ));
            }
        }

        // Check for missing required indexes
        // Note: Index is idx_user_profile_email (singular), not idx_user_profiles_email
        let required_indexes = vec![
            "idx_accounts_user_id",
            "idx_transactions_account_id",
            "idx_transactions_date",
            "idx_user_profile_email",
        ];

        for index_name in required_indexes {
            let mut rows = self
                .connection
                .query(
                    "SELECT name FROM sqlite_master WHERE type='index' AND name=?",
                    libsql::params![index_name],
                )
                .await?;

            if rows.next().await?.is_none() {
                warnings.push(format!("Missing recommended index: {index_name}"));
            }
        }

        // Check data consistency
        self.validate_data_consistency(&mut issues, &mut warnings)
            .await?;

        let is_valid = issues.is_empty();

        Ok(SchemaValidationResult {
            is_valid,
            issues,
            warnings,
            checked_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Validate data consistency
    ///
    /// Note: Uses `&mut Vec<String>` for issues/warnings to allow accumulation
    /// across multiple validation methods. This pattern is consistent with
    /// other validation methods in this module.
    #[allow(clippy::ptr_arg)]
    async fn validate_data_consistency(
        &self,
        _issues: &mut Vec<String>,
        warnings: &mut Vec<String>,
    ) -> Result<()> {
        // Check for invalid currency codes
        let mut rows = self.connection.query(
            "SELECT DISTINCT currency FROM accounts WHERE currency NOT IN ('USD', 'EUR', 'GBP', 'CAD', 'JPY')",
            (),
        ).await?;

        let mut invalid_currencies = Vec::new();
        while let Some(row) = rows.next().await? {
            let currency: String = row.get(0)?;
            invalid_currencies.push(currency);
        }

        if !invalid_currencies.is_empty() {
            warnings.push(format!(
                "Found accounts with non-standard currencies: {invalid_currencies:?}"
            ));
        }

        // Check for transactions with invalid amounts
        let mut rows = self
            .connection
            .query(
                "SELECT COUNT(*) FROM transactions WHERE amount = 0 OR amount IS NULL",
                (),
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let invalid_amount_count: i64 = row.get(0)?;
            if invalid_amount_count > 0 {
                warnings.push(format!(
                    "Found {invalid_amount_count} transactions with zero or null amounts"
                ));
            }
        }

        // Check for future-dated transactions
        let mut rows = self
            .connection
            .query(
                "SELECT COUNT(*) FROM transactions WHERE date > datetime('now')",
                (),
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let future_count: i64 = row.get(0)?;
            if future_count > 0 {
                warnings.push(format!(
                    "Found {future_count} transactions with future dates"
                ));
            }
        }

        Ok(())
    }

    /// Store financial report locally (uses reports table - BlockID)
    pub async fn store_financial_report(&self, report: &FinancialReport) -> FreshCreditResult<()> {
        info!("Storing financial report locally for user: {}", report.user_id);

        let data = serde_json::to_string(report)
            .map_err(|e| freshcredit_types::FreshCreditError::InternalError(e.to_string()))?;

        self.connection.execute(
            "INSERT OR REPLACE INTO reports (id, user_id, report_type, report_status, report_data, raw_report_data, blockchain_hash, created_at, updated_at)
             VALUES (?, ?, 'financial', 'ready', ?, ?, ?, datetime('now'), datetime('now'))",
            libsql::params![
                report.id.to_string(),
                report.user_id.clone(),
                data.clone(),
                data,
                report.blockchain_hash.clone(),
            ],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Retrieve financial report from local storage using raw SQL
    pub async fn get_financial_report(
        &self,
        user_id: &UserId,
    ) -> FreshCreditResult<Option<FinancialReport>> {
        info!("Retrieving financial report from local storage for user: {}", user_id);

        let mut rows = self.connection.query(
            "SELECT report_data FROM reports WHERE user_id = ? ORDER BY created_at DESC LIMIT 1",
            libsql::params![user_id.clone()],
        ).await
        .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?
        {
            let data: String = row
                .get(0)
                .map_err(|e| freshcredit_types::FreshCreditError::DatabaseError(e.to_string()))?;
            let report: FinancialReport = serde_json::from_str(&data)
                .map_err(|e| freshcredit_types::FreshCreditError::InternalError(e.to_string()))?;
            Ok(Some(report))
        } else {
            Ok(None)
        }
    }


    /// Save an uploaded file for AI analysis
    ///
    /// Uses SaveUploadedFileParams struct to consolidate parameters
    pub async fn save_uploaded_file(&self, params: &SaveUploadedFileParams<'_>) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        // Files expire after 24 hours
        let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
        // Derive file_type from mime_type
        let file_type = params
            .mime_type
            .split('/')
            .next()
            .unwrap_or("unknown")
            .to_string();

        self.connection.execute(
            "INSERT INTO uploaded_files (id, user_id, filename, file_type, file_size, mime_type, file_data, text_content, conversation_id, created_at, updated_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                id.clone(),
                params.user_id,
                params.filename,
                file_type,
                params.file_size,
                params.mime_type,
                params.file_data.clone().map(libsql::Value::Blob).unwrap_or(libsql::Value::Null),
                params.text_content.map(|s| s.to_string()),
                params.conversation_id.map(|s| s.to_string()),
                now.clone(),
                now,
                expires_at
            ],
        ).await?;

        Ok(id)
    }

    /// Get an uploaded file by ID
    pub async fn get_uploaded_file(&self, file_id: &str) -> Result<Option<UploadedFile>> {
        let mut rows = self.connection.query(
            "SELECT id, user_id, filename, mime_type, file_size, file_data, text_content, ai_analysis, conversation_id, created_at, expires_at
             FROM uploaded_files WHERE id = ?",
            libsql::params![file_id],
        ).await?;

        if let Some(row) = rows.next().await? {
            Ok(Some(UploadedFile {
                id: row.get(0)?,
                user_id: row.get(1)?,
                filename: row.get(2)?,
                mime_type: row.get(3)?,
                file_size: row.get(4)?,
                file_data: row.get::<Option<Vec<u8>>>(5)?,
                text_content: row.get(6)?,
                ai_analysis: row.get(7)?,
                conversation_id: row.get(8)?,
                created_at: row.get(9)?,
                expires_at: row.get(10)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Update AI analysis for an uploaded file
    pub async fn update_file_ai_analysis(&self, file_id: &str, analysis: &str) -> Result<()> {
        self.connection
            .execute(
                "UPDATE uploaded_files SET ai_analysis = ? WHERE id = ?",
                libsql::params![analysis, file_id],
            )
            .await?;
        Ok(())
    }

    /// Delete expired files
    pub async fn cleanup_expired_files(&self) -> Result<u64> {
        let now = chrono::Utc::now().to_rfc3339();
        let affected = self
            .connection
            .execute(
                "DELETE FROM uploaded_files WHERE expires_at < ?",
                libsql::params![now],
            )
            .await?;
        Ok(affected)
    }

    /// Execute a raw SQL query and return rows
    pub async fn query(&self, sql: &str, params: Vec<libsql::Value>) -> Result<libsql::Rows> {
        self.connection
            .query(sql, params)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// Execute a raw SQL statement and return affected rows count
    pub async fn execute(&self, sql: &str, params: Vec<libsql::Value>) -> Result<u64> {
        self.connection
            .execute(sql, params)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }


}
// ============================================================================
// AI Conversation Memory Structs
// ============================================================================

/// AI Conversation record
#[derive(Debug, Clone)]
pub struct AiConversation {
    pub id: String,
    pub user_id: String,
    pub title: Option<String>,
    pub context: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// AI Message record
#[derive(Debug, Clone)]
pub struct AiMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub file_attachment_id: Option<String>,
    pub tokens_used: Option<u32>,
    pub model: Option<String>,
    pub created_at: String,
}

// ============================================================================
// Scoring Model Structs (BlockScore)
// NOTE: FreshCredit does NOT generate scores - providers define their own models
// ============================================================================

/// Provider-defined scoring model record
/// COMPLIANCE: §3 - Scoring logic is owned and defined by the provider, not FreshCredit
#[derive(Debug, Clone)]
pub struct ScoringModelRecord {
    pub id: String,
    pub user_id: String,
    pub provider_id: String,
    pub score_model_id: String,
    pub score_model_name: Option<String>,
    pub score_model_version: Option<String>,
    pub data_elements_used: Option<String>,
    pub data_element_weights: Option<String>,
    pub raw_score_data: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

// ============================================================================
// Workflow Structs (Flow Builders)
// ============================================================================

/// Workflow record for flow builders
#[derive(Debug, Clone)]
pub struct WorkflowRecord {
    pub id: String,
    pub user_id: String,
    pub workflow_type: String,
    pub workflow_name: String,
    pub workflow_description: Option<String>,
    pub workflow_status: Option<String>,
    pub workflow_data: String,
    pub trigger_type: Option<String>,
    pub trigger_config: Option<String>,
    pub is_active: bool,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub run_count: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

// ============================================================================
// Webhook Event Structs (Outbox Pattern)
// ============================================================================

/// Webhook event record for outbox pattern
#[derive(Debug, Clone)]
pub struct WebhookEvent {
    pub id: String,
    pub user_id: Option<String>,
    pub provider: String,
    pub event_type: String,
    pub event_id: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub retry_count: i64,
    pub processed_at: Option<String>,
    pub created_at: String,
}

impl WebhookEvent {
    /// Create a new pending webhook event
    pub fn new_pending(
        provider: &str,
        event_type: &str,
        event_id: &str,
        payload: serde_json::Value,
        user_id: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            provider: provider.to_string(),
            event_type: event_type.to_string(),
            event_id: event_id.to_string(),
            payload,
            status: "pending".to_string(),
            retry_count: 0,
            processed_at: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Webhook event counts for admin dashboard
#[derive(Debug, Clone, Default)]
pub struct WebhookEventCounts {
    pub pending: u64,
    pub processing: u64,
    pub processed: u64,
    pub failed: u64,
    pub dead_letter: u64,
}

impl WebhookEventCounts {
    pub fn total(&self) -> u64 {
        self.pending + self.processing + self.processed + self.failed + self.dead_letter
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that the schema contains exactly 73 tables as documented
    /// HARDCODED_SCHEMA: 73 tables - update if schema changes (58 + 6 LinkedIn + 6 HealthKit + 3 Correlation)
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
        // HARDCODED_SCHEMA: 73 tables (58 + 6 LinkedIn + 6 HealthKit + 3 Correlation)
        assert_eq!(count, 73, "Schema should contain exactly 73 tables");
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
}
