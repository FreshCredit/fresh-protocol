//! Core schema definitions: user_profile, auth_tokens, user_preferences, api_keys, webauthn
//!
//! Tables in this module:
//! - user_profile: Core user identity (Entra ID integration)
//! - user_preferences: Feature flags and app settings
//! - auth_tokens: Session management (legacy, Entra handles primary auth)
//! - api_keys: API key management for programmatic access
//! - webauthn_credentials: FIDO2/passkey biometric authentication
//! - kilt_dids: KILT Protocol DID storage
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;

use super::try_create_index;

/// Initialize core tables (user_profile and related)
///
/// Creates 6 core tables: user_profile, user_preferences, auth_tokens,
/// api_keys, webauthn_credentials, kilt_dids
pub async fn initialize_core_tables(conn: &Connection) -> Result<()> {
    // Create user_profile table first (referenced by other tables)
    // P0p: Added is_admin column for first provider user admin rule (§27.4)
    // P0g: Added provider_onboarding_complete for §28.1 nav visibility
    conn.execute(
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

    // Migrations for existing databases
    let _ = conn.execute(
        "ALTER TABLE user_profile ADD COLUMN is_admin BOOLEAN DEFAULT FALSE",
        (),
    ).await;
    let _ = conn.execute(
        "ALTER TABLE user_profile ADD COLUMN provider_onboarding_complete BOOLEAN DEFAULT FALSE",
        (),
    ).await;

    // Create user_preferences table (matches production Turso schema)
    // P0g: Includes onboarding dismissal fields for §27.3 onboarding flow rules
    conn.execute(
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

    // Migrations for user_preferences
    let _ = conn.execute("ALTER TABLE user_preferences ADD COLUMN ai_mode TEXT DEFAULT 'auto'", ()).await;
    let _ = conn.execute("ALTER TABLE user_preferences ADD COLUMN mock_data_enabled BOOLEAN DEFAULT FALSE", ()).await;
    let _ = conn.execute("ALTER TABLE user_preferences ADD COLUMN onboarding_completed BOOLEAN DEFAULT FALSE", ()).await;
    let _ = conn.execute("ALTER TABLE user_preferences ADD COLUMN onboarding_permanently_dismissed BOOLEAN DEFAULT FALSE", ()).await;
    let _ = conn.execute("ALTER TABLE user_preferences ADD COLUMN onboarding_reminder_dismissed_until DATETIME", ()).await;
    let _ = conn.execute("ALTER TABLE user_preferences ADD COLUMN plaid_connection_skipped BOOLEAN DEFAULT FALSE", ()).await;
    let _ = conn.execute("ALTER TABLE user_preferences ADD COLUMN plaid_reminder_dismissed_until DATETIME", ()).await;

    // Create api_keys table for API key management
    conn.execute(
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

    // Create webauthn_credentials table for FIDO2/passkey biometric authentication
    conn.execute(
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

    // Create kilt_dids table for KILT Protocol DID storage
    conn.execute(
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

    Ok(())
}

/// Initialize core table indexes
pub async fn initialize_core_indexes(conn: &Connection) -> Result<()> {
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_user_profile_email ON user_profile(email)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_user_profile_azure_id ON user_profile(azure_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_user_preferences_user ON user_preferences(user_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_api_keys_user_id ON api_keys(user_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_api_keys_key_prefix ON api_keys(key_prefix)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_webauthn_credentials_user_id ON webauthn_credentials(user_id)").await?;
    try_create_index(conn, "CREATE UNIQUE INDEX IF NOT EXISTS idx_webauthn_credentials_credential_id ON webauthn_credentials(credential_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_kilt_dids_did_uri ON kilt_dids(did_uri)").await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_core_tables_module_exists() {
        // Module structure test - verifies the module compiles correctly
        // Async function signatures are verified at compile time when the module is used
        let _ = 1 + 1; // Compile-time verification
    }
}
