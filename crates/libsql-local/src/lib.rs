//! Local LibSQL database operations for FreshCredit
//!
//! This module implements the unified database schema for FreshCredit,
//! // HARDCODED_SCHEMA: 53 tables - update if schema changes
//! containing 53 tables that support:
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
//! Schema Version: unified-v1 (2025-12-05)

use anyhow::Result;
use freshcredit_types::{CreditReport, FreshCreditResult, UserId};
use serde::{Deserialize, Serialize};
use tracing::info;

/// User profile for database storage (unified schema)
/// Combines Entra ID claims with extended profile and Verified ID fields
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
                account_name TEXT,
                account_type TEXT NOT NULL,
                account_subtype TEXT,
                balance_available REAL,
                balance_current REAL,
                balance_limit REAL,
                currency TEXT DEFAULT 'USD',
                currency_code TEXT DEFAULT 'USD',
                balance REAL DEFAULT 0.0,
                is_funding_source BOOLEAN DEFAULT FALSE,
                date_opened DATE,
                credit_limit DECIMAL(12,2),
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                (),
            )
            .await?;

        // Create transactions table that matches production schema
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
                raw_transaction_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
                FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE
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
                raw_holding_data TEXT NOT NULL,
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
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
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

        // HARDCODED_SCHEMA: 54 tables - update if schema changes
        info!("Unified database schema initialization completed (54 tables)");
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

    /// Store report locally (uses reports table - BlockID)
    pub async fn store_credit_report(&self, report: &CreditReport) -> FreshCreditResult<()> {
        info!("Storing report locally for user: {}", report.user_id);

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

    /// Retrieve report from local storage using raw SQL
    pub async fn get_credit_report(
        &self,
        user_id: &UserId,
    ) -> FreshCreditResult<Option<CreditReport>> {
        info!("Retrieving report from local storage for user: {}", user_id);

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
            let report: CreditReport = serde_json::from_str(&data)
                .map_err(|e| freshcredit_types::FreshCreditError::InternalError(e.to_string()))?;
            Ok(Some(report))
        } else {
            Ok(None)
        }
    }

    /// Store user profile in local database (matches production schema)
    pub async fn store_user_profile(&self, profile: &UserProfile) -> Result<()> {
        info!(
            "Storing user profile locally for user: {}",
            profile.platform_user_id
        );

        self.connection.execute(
            "INSERT OR REPLACE INTO user_profile (
                id, platform_user_id, azure_id, email, display_name, given_name, family_name,
                surname, mobile_phone, job_title, street_address, city, state_province,
                postal_code, country_region, date_of_birth, ssn_last_four, employment_status,
                annual_income, role, tenant_id, object_id, verified_id_credential_id,
                verified_id_status, verified_id_issued_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))",
            libsql::params![
                profile.id.clone(),
                profile.platform_user_id.clone(),
                profile.azure_id.clone(),
                profile.email.clone(),
                profile.display_name.clone(),
                profile.given_name.clone().unwrap_or_default(),
                profile.family_name.clone().unwrap_or_default(),
                profile.surname.clone().unwrap_or_default(),
                profile.mobile_phone.clone().unwrap_or_default(),
                profile.job_title.clone().unwrap_or_default(),
                profile.street_address.clone().unwrap_or_default(),
                profile.city.clone().unwrap_or_default(),
                profile.state_province.clone().unwrap_or_default(),
                profile.postal_code.clone().unwrap_or_default(),
                profile.country_region.clone().unwrap_or_default(),
                profile.date_of_birth.clone().unwrap_or_default(),
                profile.ssn_last_four.clone().unwrap_or_default(),
                profile.employment_status.clone().unwrap_or_default(),
                profile.annual_income.unwrap_or(0),
                profile.role.clone(),
                profile.tenant_id.clone(),
                profile.object_id.clone(),
                profile.verified_id_credential_id.clone().unwrap_or_default(),
                profile.verified_id_status.clone(),
                profile.verified_id_issued_at.clone().unwrap_or_default(),
            ],
        ).await?;

        Ok(())
    }

    /// Get user profile from local database (matches production schema)
    pub async fn get_user_profile(&self, platform_user_id: &str) -> Result<Option<UserProfile>> {
        info!(
            "Retrieving user profile locally for user: {}",
            platform_user_id
        );

        let mut rows = self
            .connection
            .query(
                "SELECT * FROM user_profile WHERE platform_user_id = ?",
                libsql::params![platform_user_id],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let profile = UserProfile {
                id: row.get(0)?,
                platform_user_id: row.get(1)?,
                azure_id: row.get(2)?,
                email: row.get(3)?,
                display_name: row.get(4)?,
                given_name: row.get::<Option<String>>(5).unwrap_or(None),
                family_name: row.get::<Option<String>>(6).unwrap_or(None),
                surname: row.get::<Option<String>>(7).unwrap_or(None),
                mobile_phone: row.get::<Option<String>>(8).unwrap_or(None),
                job_title: row.get::<Option<String>>(9).unwrap_or(None),
                street_address: row.get::<Option<String>>(10).unwrap_or(None),
                city: row.get::<Option<String>>(11).unwrap_or(None),
                state_province: row.get::<Option<String>>(12).unwrap_or(None),
                postal_code: row.get::<Option<String>>(13).unwrap_or(None),
                country_region: row.get::<Option<String>>(14).unwrap_or(None),
                date_of_birth: row.get::<Option<String>>(15).unwrap_or(None),
                ssn_last_four: row.get::<Option<String>>(16).unwrap_or(None),
                employment_status: row.get::<Option<String>>(17).unwrap_or(None),
                annual_income: row.get::<Option<i32>>(18).unwrap_or(None),
                role: row.get(19).unwrap_or_else(|_| "consumer".to_string()),
                tenant_id: row.get(20)?,
                object_id: row.get(21)?,
                verified_id_credential_id: row.get::<Option<String>>(22).unwrap_or(None),
                verified_id_status: row.get(23).unwrap_or_else(|_| "pending".to_string()),
                verified_id_issued_at: row.get::<Option<String>>(24).unwrap_or(None),
                created_at: row.get(25).unwrap_or_default(),
                updated_at: row.get(26).unwrap_or_default(),
            };

            Ok(Some(profile))
        } else {
            Ok(None)
        }
    }

    /// Get user profile by Azure AD Object ID (used when user_id is the Azure ID)
    pub async fn get_user_profile_by_azure_id(
        &self,
        azure_id: &str,
    ) -> Result<Option<UserProfile>> {
        info!("Retrieving user profile by azure_id: {}", azure_id);

        let mut rows = self
            .connection
            .query(
                "SELECT * FROM user_profile WHERE azure_id = ?",
                libsql::params![azure_id],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let profile = UserProfile {
                id: row.get(0)?,
                platform_user_id: row.get(1)?,
                azure_id: row.get(2)?,
                email: row.get(3)?,
                display_name: row.get(4)?,
                given_name: row.get::<Option<String>>(5).unwrap_or(None),
                family_name: row.get::<Option<String>>(6).unwrap_or(None),
                surname: row.get::<Option<String>>(7).unwrap_or(None),
                mobile_phone: row.get::<Option<String>>(8).unwrap_or(None),
                job_title: row.get::<Option<String>>(9).unwrap_or(None),
                street_address: row.get::<Option<String>>(10).unwrap_or(None),
                city: row.get::<Option<String>>(11).unwrap_or(None),
                state_province: row.get::<Option<String>>(12).unwrap_or(None),
                postal_code: row.get::<Option<String>>(13).unwrap_or(None),
                country_region: row.get::<Option<String>>(14).unwrap_or(None),
                date_of_birth: row.get::<Option<String>>(15).unwrap_or(None),
                ssn_last_four: row.get::<Option<String>>(16).unwrap_or(None),
                employment_status: row.get::<Option<String>>(17).unwrap_or(None),
                annual_income: row.get::<Option<i32>>(18).unwrap_or(None),
                role: row.get(19).unwrap_or_else(|_| "consumer".to_string()),
                tenant_id: row.get(20)?,
                object_id: row.get(21)?,
                verified_id_credential_id: row.get::<Option<String>>(22).unwrap_or(None),
                verified_id_status: row.get(23).unwrap_or_else(|_| "pending".to_string()),
                verified_id_issued_at: row.get::<Option<String>>(24).unwrap_or(None),
                created_at: row.get(25).unwrap_or_default(),
                updated_at: row.get(26).unwrap_or_default(),
            };

            Ok(Some(profile))
        } else {
            Ok(None)
        }
    }

    /// Store account data in local database
    pub async fn store_account(&self, account: &freshcredit_types::Account) -> Result<()> {
        info!("Storing account locally: {}", account.id);

        self.connection.execute(
            "INSERT OR REPLACE INTO accounts (id, user_id, account_type, balance, currency, institution_name, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                account.id.clone(),
                account.user_id.clone(),
                format!("{:?}", account.account_type), // Convert enum to string
                account.balance,
                account.currency.clone(),
                account.institution_name.clone(),
                account.created_at.to_rfc3339(),
            ],
        ).await?;

        Ok(())
    }

    /// Store transaction data in local database
    pub async fn store_transaction(
        &self,
        transaction: &freshcredit_types::Transaction,
    ) -> Result<()> {
        info!("Storing transaction locally: {}", transaction.id);

        self.connection.execute(
            "INSERT OR REPLACE INTO transactions (id, account_id, amount, currency, description, category, date, merchant_name)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                transaction.id.clone(),
                transaction.account_id.clone(),
                transaction.amount,
                transaction.currency.clone(),
                transaction.description.clone(),
                transaction.category.clone().unwrap_or_default(),
                transaction.date.to_rfc3339(),
                transaction.merchant_name.clone().unwrap_or_default(),
            ],
        ).await?;

        Ok(())
    }

    /// Get all accounts for a user
    pub async fn get_user_accounts(
        &self,
        user_id: &str,
    ) -> Result<Vec<freshcredit_types::Account>> {
        info!("Retrieving accounts for user: {}", user_id);

        let mut rows = self
            .connection
            .query(
                "SELECT * FROM accounts WHERE user_id = ? ORDER BY created_at DESC",
                libsql::params![user_id],
            )
            .await?;

        let mut accounts = Vec::new();
        while let Some(row) = rows.next().await? {
            let account_type_str: String = row.get(2)?;
            let account_type = match account_type_str.as_str() {
                "Checking" => freshcredit_types::AccountType::Checking,
                "Savings" => freshcredit_types::AccountType::Savings,
                "Credit" => freshcredit_types::AccountType::Credit,
                "Investment" => freshcredit_types::AccountType::Investment,
                _ => freshcredit_types::AccountType::Checking, // Default fallback
            };

            let account = freshcredit_types::Account {
                id: row.get(0)?,
                user_id: row.get(1)?,
                account_type,
                balance: row.get(3)?,
                currency: row.get(4)?,
                institution_name: row.get(5)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String>(6)?)
                    .map_err(|e| anyhow::anyhow!("Failed to parse date: {e}"))?
                    .with_timezone(&chrono::Utc),
            };
            accounts.push(account);
        }

        Ok(accounts)
    }

    /// Get all transactions for a user
    pub async fn get_user_transactions(
        &self,
        user_id: &str,
    ) -> Result<Vec<freshcredit_types::Transaction>> {
        info!("Retrieving transactions for user: {user_id}");

        let mut rows = self
            .connection
            .query(
                "SELECT t.* FROM transactions t
             JOIN accounts a ON t.account_id = a.id
             WHERE a.user_id = ?
             ORDER BY t.date DESC",
                libsql::params![user_id],
            )
            .await?;

        let mut transactions = Vec::new();
        while let Some(row) = rows.next().await? {
            let category = row.get::<String>(5)?;
            let merchant_name = row.get::<String>(7)?;

            let transaction = freshcredit_types::Transaction {
                id: row.get(0)?,
                account_id: row.get(1)?,
                amount: row.get(2)?,
                currency: row.get(3)?,
                description: row.get(4)?,
                category: if category.is_empty() {
                    None
                } else {
                    Some(category)
                },
                date: chrono::DateTime::parse_from_rfc3339(&row.get::<String>(6)?)
                    .map_err(|e| anyhow::anyhow!("Failed to parse date: {e}"))?
                    .with_timezone(&chrono::Utc),
                merchant_name: if merchant_name.is_empty() {
                    None
                } else {
                    Some(merchant_name)
                },
            };
            transactions.push(transaction);
        }

        Ok(transactions)
    }

    /// Get user preferences
    pub async fn get_user_preferences(&self, user_id: &str) -> Result<Option<UserPreferences>> {
        info!("Getting preferences for user: {user_id}");

        let mut rows = self.connection.query(
            "SELECT ai_agent_enabled, ai_feedback_enabled, ai_offers_enabled, ai_lenders_enabled,
                    cloud_sync_enabled, blockchain_enabled, email_notifications_enabled, kilt_did_enabled,
                    COALESCE(ai_mode, 'auto') as ai_mode, COALESCE(mock_data_enabled, 0) as mock_data_enabled
             FROM user_preferences WHERE user_id = ?",
            libsql::params![user_id],
        ).await?;

        if let Some(row) = rows.next().await? {
            Ok(Some(UserPreferences {
                ai_agent_enabled: Some(row.get::<i64>(0)? != 0),
                ai_feedback_enabled: Some(row.get::<i64>(1)? != 0),
                ai_offers_enabled: Some(row.get::<i64>(2)? != 0),
                ai_lenders_enabled: Some(row.get::<i64>(3)? != 0),
                cloud_sync_enabled: Some(row.get::<i64>(4)? != 0),
                blockchain_enabled: Some(row.get::<i64>(5)? != 0),
                email_notifications_enabled: Some(row.get::<i64>(6)? != 0),
                kilt_did_enabled: Some(row.get::<i64>(7)? != 0),
                ai_mode: Some(row.get::<String>(8)?),
                mock_data_enabled: Some(row.get::<i64>(9)? != 0),
            }))
        } else {
            Ok(None)
        }
    }

    /// Save user preferences
    pub async fn save_user_preferences(
        &self,
        user_id: &str,
        prefs: &UserPreferences,
    ) -> Result<()> {
        info!("Saving preferences for user: {user_id}");

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        self.connection
            .execute(
                "INSERT INTO user_preferences (id, user_id, ai_agent_enabled, ai_feedback_enabled,
                ai_offers_enabled, ai_lenders_enabled, cloud_sync_enabled, blockchain_enabled,
                email_notifications_enabled, kilt_did_enabled, ai_mode, mock_data_enabled,
                created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(user_id) DO UPDATE SET
                ai_agent_enabled = excluded.ai_agent_enabled,
                ai_feedback_enabled = excluded.ai_feedback_enabled,
                ai_offers_enabled = excluded.ai_offers_enabled,
                ai_lenders_enabled = excluded.ai_lenders_enabled,
                cloud_sync_enabled = excluded.cloud_sync_enabled,
                blockchain_enabled = excluded.blockchain_enabled,
                email_notifications_enabled = excluded.email_notifications_enabled,
                kilt_did_enabled = excluded.kilt_did_enabled,
                ai_mode = excluded.ai_mode,
                mock_data_enabled = excluded.mock_data_enabled,
                updated_at = excluded.updated_at",
                libsql::params![
                    id,
                    user_id,
                    prefs.ai_agent_enabled.unwrap_or(false) as i64,
                    prefs.ai_feedback_enabled.unwrap_or(false) as i64,
                    prefs.ai_offers_enabled.unwrap_or(false) as i64,
                    prefs.ai_lenders_enabled.unwrap_or(false) as i64,
                    prefs.cloud_sync_enabled.unwrap_or(true) as i64,
                    prefs.blockchain_enabled.unwrap_or(true) as i64,
                    prefs.email_notifications_enabled.unwrap_or(true) as i64,
                    prefs.kilt_did_enabled.unwrap_or(false) as i64,
                    prefs.ai_mode.clone().unwrap_or_else(|| "auto".to_string()),
                    prefs.mock_data_enabled.unwrap_or(false) as i64,
                    now.clone(),
                    now
                ],
            )
            .await?;

        Ok(())
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

    // ========================================================================
    // AI Conversation Memory Functions
    // ========================================================================

    /// Create a new conversation
    pub async fn create_conversation(
        &self,
        user_id: &str,
        title: Option<&str>,
        context: Option<&str>,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        self.connection
            .execute(
                "INSERT INTO ai_conversations (id, user_id, title, context, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
                libsql::params![id.clone(), user_id, title, context, now.clone(), now],
            )
            .await?;

        Ok(id)
    }

    /// Get conversation by ID
    pub async fn get_conversation(&self, conversation_id: &str) -> Result<Option<AiConversation>> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, user_id, title, context, created_at, updated_at
             FROM ai_conversations WHERE id = ?",
                libsql::params![conversation_id],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            Ok(Some(AiConversation {
                id: row.get(0)?,
                user_id: row.get(1)?,
                title: row.get(2)?,
                context: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get recent conversations for a user
    pub async fn get_user_conversations(
        &self,
        user_id: &str,
        limit: u32,
    ) -> Result<Vec<AiConversation>> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, user_id, title, context, created_at, updated_at
             FROM ai_conversations WHERE user_id = ?
             ORDER BY updated_at DESC LIMIT ?",
                libsql::params![user_id, limit as i64],
            )
            .await?;

        let mut conversations = Vec::new();
        while let Some(row) = rows.next().await? {
            conversations.push(AiConversation {
                id: row.get(0)?,
                user_id: row.get(1)?,
                title: row.get(2)?,
                context: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            });
        }
        Ok(conversations)
    }

    /// Add a message to a conversation
    pub async fn add_conversation_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        file_attachment_id: Option<&str>,
        tokens_used: Option<u32>,
        model: Option<&str>,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        self.connection.execute(
            "INSERT INTO ai_messages (id, conversation_id, role, content, file_attachment_id, tokens_used, model, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                id.clone(),
                conversation_id,
                role,
                content,
                file_attachment_id,
                tokens_used.map(|t| t as i64),
                model,
                now.clone()
            ],
        ).await?;

        // Update conversation's updated_at timestamp
        self.connection
            .execute(
                "UPDATE ai_conversations SET updated_at = ? WHERE id = ?",
                libsql::params![now, conversation_id],
            )
            .await?;

        Ok(id)
    }

    /// Get messages for a conversation
    pub async fn get_conversation_messages(
        &self,
        conversation_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<AiMessage>> {
        let query = if let Some(lim) = limit {
            format!(
                "SELECT id, conversation_id, role, content, file_attachment_id, tokens_used, model, created_at
                 FROM ai_messages WHERE conversation_id = ?
                 ORDER BY created_at ASC LIMIT {lim}"
            )
        } else {
            "SELECT id, conversation_id, role, content, file_attachment_id, tokens_used, model, created_at
             FROM ai_messages WHERE conversation_id = ?
             ORDER BY created_at ASC".to_string()
        };

        let mut rows = self
            .connection
            .query(&query, libsql::params![conversation_id])
            .await?;

        let mut messages = Vec::new();
        while let Some(row) = rows.next().await? {
            messages.push(AiMessage {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                file_attachment_id: row.get(4)?,
                tokens_used: row.get::<Option<i64>>(5)?.map(|t| t as u32),
                model: row.get(6)?,
                created_at: row.get(7)?,
            });
        }
        Ok(messages)
    }

    /// Delete a conversation and all its messages
    pub async fn delete_conversation(&self, conversation_id: &str) -> Result<()> {
        self.connection
            .execute(
                "DELETE FROM ai_conversations WHERE id = ?",
                libsql::params![conversation_id],
            )
            .await?;
        Ok(())
    }

    /// Update conversation title (auto-generated from first message)
    pub async fn update_conversation_title(
        &self,
        conversation_id: &str,
        title: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.connection
            .execute(
                "UPDATE ai_conversations SET title = ?, updated_at = ? WHERE id = ?",
                libsql::params![title, now, conversation_id],
            )
            .await?;
        Ok(())
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

    // ========================================================================
    // Scoring Model Operations (BlockScore)
    // NOTE: FreshCredit does NOT generate scores - providers define their own models
    // ========================================================================

    /// Save a provider-defined scoring model
    pub async fn save_scoring_model(&self, model: &ScoringModelRecord) -> Result<()> {
        info!(
            "Saving scoring model: {} for provider: {}",
            model.id, model.provider_id
        );

        let now = chrono::Utc::now().to_rfc3339();
        self.connection
            .execute(
                "INSERT OR REPLACE INTO scores (
                id, user_id, provider_id, score_model_id, score_model_name,
                score_model_version, data_elements_used, data_element_weights,
                raw_score_data, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                libsql::params![
                    model.id.clone(),
                    model.user_id.clone(),
                    model.provider_id.clone(),
                    model.score_model_id.clone(),
                    model.score_model_name.clone().unwrap_or_default(),
                    model.score_model_version.clone().unwrap_or_default(),
                    model.data_elements_used.clone().unwrap_or_default(),
                    model.data_element_weights.clone().unwrap_or_default(),
                    model.raw_score_data.clone(),
                    now.clone(),
                    now
                ],
            )
            .await?;

        Ok(())
    }

    /// Get scoring models for a provider
    pub async fn get_scoring_models(&self, provider_id: &str) -> Result<Vec<ScoringModelRecord>> {
        info!("Getting scoring models for provider: {}", provider_id);

        let mut rows = self
            .connection
            .query(
                "SELECT id, user_id, provider_id, score_model_id, score_model_name,
                    score_model_version, data_elements_used, data_element_weights,
                    raw_score_data, created_at, updated_at
             FROM scores WHERE provider_id = ? ORDER BY updated_at DESC",
                libsql::params![provider_id],
            )
            .await?;

        let mut models = Vec::new();
        while let Some(row) = rows.next().await? {
            models.push(ScoringModelRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                provider_id: row.get(2)?,
                score_model_id: row.get(3)?,
                score_model_name: row.get(4)?,
                score_model_version: row.get(5)?,
                data_elements_used: row.get(6)?,
                data_element_weights: row.get(7)?,
                raw_score_data: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            });
        }

        Ok(models)
    }

    /// Get a specific scoring model by ID
    pub async fn get_scoring_model(&self, model_id: &str) -> Result<Option<ScoringModelRecord>> {
        info!("Getting scoring model: {}", model_id);

        let mut rows = self
            .connection
            .query(
                "SELECT id, user_id, provider_id, score_model_id, score_model_name,
                    score_model_version, data_elements_used, data_element_weights,
                    raw_score_data, created_at, updated_at
             FROM scores WHERE id = ?",
                libsql::params![model_id],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            Ok(Some(ScoringModelRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                provider_id: row.get(2)?,
                score_model_id: row.get(3)?,
                score_model_name: row.get(4)?,
                score_model_version: row.get(5)?,
                data_elements_used: row.get(6)?,
                data_element_weights: row.get(7)?,
                raw_score_data: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Delete a scoring model
    pub async fn delete_scoring_model(&self, model_id: &str) -> Result<bool> {
        info!("Deleting scoring model: {}", model_id);

        let affected = self
            .connection
            .execute("DELETE FROM scores WHERE id = ?", libsql::params![model_id])
            .await?;

        Ok(affected > 0)
    }

    // ========================================================================
    // WORKFLOW OPERATIONS
    // ========================================================================

    /// Save a workflow (flow builder)
    pub async fn save_workflow(&self, workflow: &WorkflowRecord) -> Result<()> {
        info!(
            "Saving workflow: {} for user: {}",
            workflow.id, workflow.user_id
        );

        let now = chrono::Utc::now().to_rfc3339();
        self.connection
            .execute(
                "INSERT OR REPLACE INTO workflows (
                id, user_id, workflow_type, workflow_name, workflow_description,
                workflow_status, workflow_data, trigger_type, trigger_config,
                is_active, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                libsql::params![
                    workflow.id.clone(),
                    workflow.user_id.clone(),
                    workflow.workflow_type.clone(),
                    workflow.workflow_name.clone(),
                    workflow.workflow_description.clone().unwrap_or_default(),
                    workflow
                        .workflow_status
                        .clone()
                        .unwrap_or_else(|| "draft".to_string()),
                    workflow.workflow_data.clone(),
                    workflow.trigger_type.clone().unwrap_or_default(),
                    workflow.trigger_config.clone().unwrap_or_default(),
                    workflow.is_active,
                    now.clone(),
                    now
                ],
            )
            .await?;

        Ok(())
    }

    /// Get all workflows for a user
    pub async fn get_workflows(&self, user_id: &str) -> Result<Vec<WorkflowRecord>> {
        info!("Getting workflows for user: {}", user_id);

        let mut rows = self
            .connection
            .query(
                "SELECT id, user_id, workflow_type, workflow_name, workflow_description,
                    workflow_status, workflow_data, trigger_type, trigger_config,
                    is_active, last_run_at, next_run_at, run_count, created_at, updated_at
             FROM workflows WHERE user_id = ? ORDER BY updated_at DESC",
                libsql::params![user_id],
            )
            .await?;

        let mut workflows = Vec::new();
        while let Some(row) = rows.next().await? {
            workflows.push(WorkflowRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                workflow_type: row.get(2)?,
                workflow_name: row.get(3)?,
                workflow_description: row.get(4)?,
                workflow_status: row.get(5)?,
                workflow_data: row.get(6)?,
                trigger_type: row.get(7)?,
                trigger_config: row.get(8)?,
                is_active: row.get::<i64>(9)? != 0,
                last_run_at: row.get(10)?,
                next_run_at: row.get(11)?,
                run_count: row.get(12)?,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
            });
        }

        Ok(workflows)
    }

    /// Get a specific workflow by ID
    pub async fn get_workflow(&self, workflow_id: &str) -> Result<Option<WorkflowRecord>> {
        info!("Getting workflow: {}", workflow_id);

        let mut rows = self
            .connection
            .query(
                "SELECT id, user_id, workflow_type, workflow_name, workflow_description,
                    workflow_status, workflow_data, trigger_type, trigger_config,
                    is_active, last_run_at, next_run_at, run_count, created_at, updated_at
             FROM workflows WHERE id = ?",
                libsql::params![workflow_id],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            Ok(Some(WorkflowRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                workflow_type: row.get(2)?,
                workflow_name: row.get(3)?,
                workflow_description: row.get(4)?,
                workflow_status: row.get(5)?,
                workflow_data: row.get(6)?,
                trigger_type: row.get(7)?,
                trigger_config: row.get(8)?,
                is_active: row.get::<i64>(9)? != 0,
                last_run_at: row.get(10)?,
                next_run_at: row.get(11)?,
                run_count: row.get(12)?,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Delete a workflow
    pub async fn delete_workflow(&self, workflow_id: &str) -> Result<bool> {
        info!("Deleting workflow: {}", workflow_id);

        let affected = self
            .connection
            .execute(
                "DELETE FROM workflows WHERE id = ?",
                libsql::params![workflow_id],
            )
            .await?;

        Ok(affected > 0)
    }

    /// Publish a workflow (set status to published and is_active to true)
    pub async fn publish_workflow(&self, workflow_id: &str) -> Result<bool> {
        info!("Publishing workflow: {}", workflow_id);

        let now = chrono::Utc::now().to_rfc3339();
        let affected = self.connection.execute(
            "UPDATE workflows SET workflow_status = 'published', is_active = 1, updated_at = ? WHERE id = ?",
            libsql::params![now, workflow_id],
        ).await?;

        Ok(affected > 0)
    }

    // ========================================================================
    // Webhook Event Methods (Outbox Pattern)
    // ========================================================================

    /// Store a webhook event for processing (outbox pattern)
    /// Returns true if stored, false if duplicate (idempotent)
    pub async fn store_webhook_event(&self, event: &WebhookEvent) -> Result<bool> {
        info!(
            "Storing webhook event: {} from {}",
            event.event_id, event.provider
        );

        let now = chrono::Utc::now().to_rfc3339();
        let payload_json = serde_json::to_string(&event.payload)?;

        // Use INSERT OR IGNORE for idempotency via event_id UNIQUE constraint
        let affected = self.connection.execute(
            "INSERT OR IGNORE INTO webhook_events (id, user_id, provider, event_type, event_id, payload, status, retry_count, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                event.id.clone(),
                event.user_id.clone(),
                event.provider.clone(),
                event.event_type.clone(),
                event.event_id.clone(),
                payload_json,
                event.status.clone(),
                event.retry_count,
                now
            ],
        ).await?;

        Ok(affected > 0)
    }

    /// Get pending webhook events for processing
    pub async fn get_pending_webhook_events(&self, limit: u32) -> Result<Vec<WebhookEvent>> {
        info!("Getting pending webhook events (limit: {})", limit);

        let mut rows = self.connection.query(
            "SELECT id, user_id, provider, event_type, event_id, payload, status, retry_count, processed_at, created_at
             FROM webhook_events
             WHERE status = 'pending'
             ORDER BY created_at ASC
             LIMIT ?",
            libsql::params![limit as i64],
        ).await?;

        let mut events = Vec::new();
        while let Some(row) = rows.next().await? {
            let payload_str: String = row.get(5)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();

            events.push(WebhookEvent {
                id: row.get(0)?,
                user_id: row.get::<Option<String>>(1)?,
                provider: row.get(2)?,
                event_type: row.get(3)?,
                event_id: row.get(4)?,
                payload,
                status: row.get(6)?,
                retry_count: row.get(7)?,
                processed_at: row.get::<Option<String>>(8)?,
                created_at: row.get(9)?,
            });
        }

        Ok(events)
    }

    /// Update webhook event status after processing
    pub async fn update_webhook_status(
        &self,
        event_id: &str,
        status: &str,
        increment_retry: bool,
    ) -> Result<bool> {
        info!("Updating webhook status: {} -> {}", event_id, status);

        let now = chrono::Utc::now().to_rfc3339();
        let processed_at = if status == "processed" || status == "failed" {
            Some(now.clone())
        } else {
            None
        };

        let affected = if increment_retry {
            self.connection.execute(
                "UPDATE webhook_events SET status = ?, retry_count = retry_count + 1, processed_at = ? WHERE event_id = ?",
                libsql::params![status, processed_at, event_id],
            ).await?
        } else {
            self.connection
                .execute(
                    "UPDATE webhook_events SET status = ?, processed_at = ? WHERE event_id = ?",
                    libsql::params![status, processed_at, event_id],
                )
                .await?
        };

        Ok(affected > 0)
    }

    /// Get webhook events that need retry (failed with retry_count < max_retries)
    pub async fn get_webhook_events_for_retry(
        &self,
        max_retries: u32,
    ) -> Result<Vec<WebhookEvent>> {
        info!(
            "Getting webhook events for retry (max_retries: {})",
            max_retries
        );

        let mut rows = self.connection.query(
            "SELECT id, user_id, provider, event_type, event_id, payload, status, retry_count, processed_at, created_at
             FROM webhook_events
             WHERE status = 'failed' AND retry_count < ?
             ORDER BY created_at ASC",
            libsql::params![max_retries as i64],
        ).await?;

        let mut events = Vec::new();
        while let Some(row) = rows.next().await? {
            let payload_str: String = row.get(5)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();

            events.push(WebhookEvent {
                id: row.get(0)?,
                user_id: row.get::<Option<String>>(1)?,
                provider: row.get(2)?,
                event_type: row.get(3)?,
                event_id: row.get(4)?,
                payload,
                status: row.get(6)?,
                retry_count: row.get(7)?,
                processed_at: row.get::<Option<String>>(8)?,
                created_at: row.get(9)?,
            });
        }

        Ok(events)
    }

    /// Move webhook to dead letter queue (max retries exceeded)
    pub async fn move_webhook_to_dead_letter(&self, event_id: &str) -> Result<bool> {
        info!("Moving webhook to dead letter: {}", event_id);

        let now = chrono::Utc::now().to_rfc3339();
        let affected = self.connection.execute(
            "UPDATE webhook_events SET status = 'dead_letter', processed_at = ? WHERE event_id = ?",
            libsql::params![now, event_id],
        ).await?;

        Ok(affected > 0)
    }

    /// Get webhook event by event_id
    pub async fn get_webhook_event(&self, event_id: &str) -> Result<Option<WebhookEvent>> {
        let mut rows = self.connection.query(
            "SELECT id, user_id, provider, event_type, event_id, payload, status, retry_count, processed_at, created_at
             FROM webhook_events WHERE event_id = ?",
            libsql::params![event_id],
        ).await?;

        if let Some(row) = rows.next().await? {
            let payload_str: String = row.get(5)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();

            Ok(Some(WebhookEvent {
                id: row.get(0)?,
                user_id: row.get::<Option<String>>(1)?,
                provider: row.get(2)?,
                event_type: row.get(3)?,
                event_id: row.get(4)?,
                payload,
                status: row.get(6)?,
                retry_count: row.get(7)?,
                processed_at: row.get::<Option<String>>(8)?,
                created_at: row.get(9)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get recent webhook events for admin dashboard
    pub async fn get_recent_webhook_events(&self, limit: u32) -> Result<Vec<WebhookEvent>> {
        let mut rows = self.connection.query(
            "SELECT id, user_id, provider, event_type, event_id, payload, status, retry_count, processed_at, created_at
             FROM webhook_events
             ORDER BY created_at DESC
             LIMIT ?",
            libsql::params![limit as i64],
        ).await?;

        let mut events = Vec::new();
        while let Some(row) = rows.next().await? {
            let payload_str: String = row.get(5)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();

            events.push(WebhookEvent {
                id: row.get(0)?,
                user_id: row.get::<Option<String>>(1)?,
                provider: row.get(2)?,
                event_type: row.get(3)?,
                event_id: row.get(4)?,
                payload,
                status: row.get(6)?,
                retry_count: row.get(7)?,
                processed_at: row.get::<Option<String>>(8)?,
                created_at: row.get(9)?,
            });
        }

        Ok(events)
    }

    /// Get webhook event counts by status for admin dashboard
    pub async fn get_webhook_event_counts(&self) -> Result<WebhookEventCounts> {
        let mut rows = self
            .connection
            .query(
                "SELECT status, COUNT(*) as count FROM webhook_events GROUP BY status",
                libsql::params![],
            )
            .await?;

        let mut counts = WebhookEventCounts::default();
        while let Some(row) = rows.next().await? {
            let status: String = row.get(0)?;
            let count: i64 = row.get(1)?;

            match status.as_str() {
                "pending" => counts.pending = count as u64,
                "processing" => counts.processing = count as u64,
                "processed" => counts.processed = count as u64,
                "failed" => counts.failed = count as u64,
                "dead_letter" => counts.dead_letter = count as u64,
                _ => {}
            }
        }

        Ok(counts)
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

    /// Test that the schema contains exactly 53 tables as documented
    /// HARDCODED_SCHEMA: 53 tables - update if schema changes
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
        // HARDCODED_SCHEMA: 54 tables - update if schema changes
        assert_eq!(count, 54, "Schema should contain exactly 54 tables");
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
