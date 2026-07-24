//! Core schema definitions: `user_profile`, `auth_tokens`, `user_preferences`, `api_keys`, webauthn
//!
//! Tables in this module:
//! - `user_profile`: Core user identity (Entra ID integration)
//! - `user_preferences`: Feature flags and app settings
//! - `auth_tokens`: Session management (legacy, Entra handles primary auth)
//! - `api_keys`: API key management for programmatic access
//! - `webauthn_credentials`: FIDO2/passkey biometric authentication
//! - `kilt_dids`: KILT Protocol DID storage
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

// TAG: surface=database owner=platform-team rule=DB-001
use anyhow::Result;
use libsql::Connection;

use super::try_create_index;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize core tables (`user_profile` and related)
///
/// Creates 6 core tables: `user_profile`, `user_preferences`, `auth_tokens`,
/// `api_keys`, `webauthn_credentials`, `kilt_dids`
/// # Errors
///
/// Returns an error if the operation fails.
#[allow(clippy::too_many_lines)]
pub async fn initialize_core_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing core tables");
    // Create user_profile table first (referenced by other tables)
    // P0p: Added is_admin column for first provider user admin rule (§27.4)
    // P0g: Added provider_onboarding_complete for §28.1 nav visibility
    // ARCH-P2-001: Added phone_number, preferred_name, emergency_contact_name,
    //              emergency_contact_phone, employer_name for web schema alignment
    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_profile (
            id TEXT PRIMARY KEY,
            platform_user_id TEXT NOT NULL DEFAULT '',
            azure_id TEXT NOT NULL DEFAULT '',
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
            phone_number TEXT,
            preferred_name TEXT,
            emergency_contact_name TEXT,
            emergency_contact_phone TEXT,
            employer_name TEXT,
            role TEXT DEFAULT 'consumer',
            is_admin BOOLEAN DEFAULT FALSE,
            provider_onboarding_complete BOOLEAN DEFAULT FALSE,
            mfa_enabled BOOLEAN DEFAULT FALSE,
            mfa_verified_at TEXT,
            tenant_id TEXT NOT NULL DEFAULT '',
            object_id TEXT NOT NULL DEFAULT '',
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

    // Linked sign-in identities (see schema_manager core.rs): provider subject ->
    // owning profile, enabling verified-email account linking across providers.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_identities (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            subject TEXT NOT NULL,
            email TEXT,
            email_verified BOOLEAN DEFAULT FALSE,
            is_primary BOOLEAN DEFAULT FALSE,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(provider, subject)
        )",
        (),
    )
    .await?;
    let _ = conn
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_user_identities_user_id ON user_identities(user_id)",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_user_identities_email ON user_identities(email)",
            (),
        )
        .await;
    // Existing databases: add the primary sign-in flag and backfill the
    // earliest identity per user (mirrors
    // migrations/2026-07-12-user-identities-primary.sql). Both statements are
    // idempotent: the ALTER errors out harmlessly once the column exists, and
    // the backfill skips users that already have a primary row.
    let _ = conn
        .execute(
            "ALTER TABLE user_identities ADD COLUMN is_primary BOOLEAN DEFAULT FALSE",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "UPDATE user_identities SET is_primary = TRUE WHERE rowid IN (\
             SELECT ui.rowid FROM user_identities ui \
             WHERE ui.rowid = (SELECT ui2.rowid FROM user_identities ui2 \
             WHERE ui2.user_id = ui.user_id ORDER BY ui2.created_at ASC, ui2.rowid ASC LIMIT 1) \
             AND NOT EXISTS (SELECT 1 FROM user_identities p WHERE p.user_id = ui.user_id AND p.is_primary))",
            (),
        )
        .await;

    // Migrations for existing databases
    // CHATBOT-FIX: platform_user_id is required by RBAC middleware queries
    // This column was added to the schema but existing databases may not have it
    let _ = conn
// TAG: surface=database owner=platform-team rule=DB-001
        .execute(
            "ALTER TABLE user_profile ADD COLUMN platform_user_id TEXT",
            (),
        )
        .await;
    // For rows with NULL platform_user_id, populate from email as fallback
    let _ = conn.execute(
        "UPDATE user_profile SET platform_user_id = email WHERE platform_user_id IS NULL OR platform_user_id = ''",
        (),
    ).await;

    let _ = conn
        .execute(
            "ALTER TABLE user_profile ADD COLUMN is_admin BOOLEAN DEFAULT FALSE",
            (),
        )
        .await;
    let _ = conn.execute(
        "ALTER TABLE user_profile ADD COLUMN provider_onboarding_complete BOOLEAN DEFAULT FALSE",
        (),
    ).await;
    // ARCH-P2-001: Migration for new profile fields
    let _ = conn
        .execute("ALTER TABLE user_profile ADD COLUMN phone_number TEXT", ())
        .await;
    let _ = conn
        .execute(
            "ALTER TABLE user_profile ADD COLUMN preferred_name TEXT",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "ALTER TABLE user_profile ADD COLUMN emergency_contact_name TEXT",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "ALTER TABLE user_profile ADD COLUMN emergency_contact_phone TEXT",
            (),
        )
        .await;
    let _ = conn
        .execute("ALTER TABLE user_profile ADD COLUMN employer_name TEXT", ())
        .await;

    // Per-side Verified ID issuance tracking: a single user may hold one
    // consumer credential and one provider credential, issued independently.
    let _ = conn
        .execute(
            "ALTER TABLE user_profile ADD COLUMN consumer_verified_id_credential_id TEXT",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "ALTER TABLE user_profile ADD COLUMN consumer_verified_id_status TEXT DEFAULT 'pending'",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "ALTER TABLE user_profile ADD COLUMN consumer_verified_id_issued_at TEXT",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "ALTER TABLE user_profile ADD COLUMN provider_verified_id_credential_id TEXT",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "ALTER TABLE user_profile ADD COLUMN provider_verified_id_status TEXT DEFAULT 'pending'",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "ALTER TABLE user_profile ADD COLUMN provider_verified_id_issued_at TEXT",
            (),
        )
        .await;

    // TAG: surface=database owner=platform-team rule=DB-001
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

    // TAG: surface=database owner=platform-team rule=DB-001
    // Migrations for user_preferences
    let _ = conn
        .execute(
            "ALTER TABLE user_preferences ADD COLUMN ai_mode TEXT DEFAULT 'auto'",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "ALTER TABLE user_preferences ADD COLUMN mock_data_enabled BOOLEAN DEFAULT FALSE",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "ALTER TABLE user_preferences ADD COLUMN onboarding_completed BOOLEAN DEFAULT FALSE",
            (),
        )
        .await;
    let _ = conn.execute("ALTER TABLE user_preferences ADD COLUMN onboarding_permanently_dismissed BOOLEAN DEFAULT FALSE", ()).await;
    let _ = conn
        .execute(
            "ALTER TABLE user_preferences ADD COLUMN onboarding_reminder_dismissed_until DATETIME",
            (),
        )
        .await;
    let _ = conn.execute("ALTER TABLE user_preferences ADD COLUMN plaid_connection_skipped BOOLEAN DEFAULT FALSE", ()).await;
    let _ = conn
        .execute(
            "ALTER TABLE user_preferences ADD COLUMN plaid_reminder_dismissed_until DATETIME",
            (),
        )
        .await;

    // Create api_keys table for API key management
    conn.execute(
        "CREATE TABLE IF NOT EXISTS api_keys (
            id TEXT PRIMARY KEY,
            user_email TEXT NOT NULL,
            name TEXT NOT NULL,
            key_hash TEXT NOT NULL,
            key_prefix TEXT NOT NULL,
            permissions TEXT NOT NULL DEFAULT 'read',
            last_used_at DATETIME,
            expires_at DATETIME,
            is_revoked BOOLEAN DEFAULT FALSE,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            revoked_at DATETIME,
            revoked_reason TEXT,
            rotated_at DATETIME,
            encrypted_value TEXT,
            service TEXT,
            environment TEXT
        )",
        (),
    )
    .await?;

    // Migration: rebuild api_keys if it was created with the old schema
    // (user_id/key_name instead of user_email/name).
    let api_keys_exists = {
        let mut rows = conn
            .query(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='api_keys'",
                (),
            )
            .await?;
        let exists = rows.next().await?.is_some();
        // Drain remaining rows so the connection can be reused.
        while rows.next().await?.is_some() {}
        exists
    };
    if api_keys_exists {
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM pragma_table_info('api_keys') WHERE name = 'user_email'",
                (),
            )
            .await?;
        let has_user_email: i64 = if let Some(row) = rows.next().await? {
            let v: i64 = row.get(0)?;
            while rows.next().await?.is_some() {}
            v
        } else {
            0
        };
        if has_user_email == 0 {
            conn.execute("ALTER TABLE api_keys RENAME TO api_keys_old", ())
                .await?;
            // Backfill any columns that may be missing on very old tables before copying.
            let _ = conn
                .execute(
                    "ALTER TABLE api_keys_old ADD COLUMN permissions TEXT DEFAULT 'read'",
                    (),
                )
                .await;
            let _ = conn
                .execute(
                    "ALTER TABLE api_keys_old ADD COLUMN is_revoked BOOLEAN DEFAULT FALSE",
                    (),
                )
                .await;
            let _ = conn
                .execute(
                    "ALTER TABLE api_keys_old ADD COLUMN last_used_at DATETIME",
                    (),
                )
                .await;
            let _ = conn
                .execute(
                    "ALTER TABLE api_keys_old ADD COLUMN expires_at DATETIME",
                    (),
                )
                .await;
            let _ = conn
                .execute(
                    "ALTER TABLE api_keys_old ADD COLUMN updated_at DATETIME",
                    (),
                )
                .await;
            conn.execute(
                "CREATE TABLE api_keys (
                    id TEXT PRIMARY KEY,
                    user_email TEXT NOT NULL,
                    name TEXT NOT NULL,
                    key_hash TEXT NOT NULL,
                    key_prefix TEXT NOT NULL,
                    permissions TEXT NOT NULL DEFAULT 'read',
                    last_used_at DATETIME,
                    expires_at DATETIME,
                    is_revoked BOOLEAN DEFAULT FALSE,
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                    revoked_at DATETIME,
                    revoked_reason TEXT,
                    rotated_at DATETIME
                )",
                (),
            )
            .await?;
            conn.execute(
                "INSERT INTO api_keys (id, user_email, name, key_hash, key_prefix, permissions, last_used_at, expires_at, is_revoked, created_at, updated_at)
                 SELECT id, user_id, key_name, key_hash, key_prefix,
                        COALESCE(permissions, 'read'),
                        last_used_at, expires_at,
                        COALESCE(is_revoked, FALSE),
                        COALESCE(created_at, CURRENT_TIMESTAMP),
                        COALESCE(updated_at, created_at, CURRENT_TIMESTAMP)
                 FROM api_keys_old",
                (),
            )
            .await?;
            conn.execute("DROP TABLE api_keys_old", ()).await?;
        }
    }

    // Ensure all columns exist for tables that may have been created with an
    // older version of the correct schema.
    let _ = conn
        .execute(
            "ALTER TABLE api_keys ADD COLUMN is_revoked BOOLEAN DEFAULT FALSE",
            (),
        )
        .await;
    let _ = conn
        .execute("ALTER TABLE api_keys ADD COLUMN last_used_at DATETIME", ())
        .await;
    let _ = conn
        .execute("ALTER TABLE api_keys ADD COLUMN expires_at DATETIME", ())
        .await;
    let _ = conn
        .execute(
            "ALTER TABLE api_keys ADD COLUMN updated_at DATETIME DEFAULT CURRENT_TIMESTAMP",
            (),
        )
        .await;
    let _ = conn
        .execute("ALTER TABLE api_keys ADD COLUMN revoked_at DATETIME", ())
        .await;
    let _ = conn
        .execute("ALTER TABLE api_keys ADD COLUMN revoked_reason TEXT", ())
        .await;
    let _ = conn
        .execute("ALTER TABLE api_keys ADD COLUMN rotated_at DATETIME", ())
        .await;
    // Credential-vault columns for retrievable service credentials
    // (encrypted_value holds AES-256-GCM ciphertext; NULL for legacy
    // hash-only API keys).
    let _ = conn
        .execute("ALTER TABLE api_keys ADD COLUMN encrypted_value TEXT", ())
        .await;
    let _ = conn
        .execute("ALTER TABLE api_keys ADD COLUMN service TEXT", ())
        .await;
    let _ = conn
        .execute("ALTER TABLE api_keys ADD COLUMN environment TEXT", ())
        .await;

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

    // TAG: surface=database owner=platform-team rule=DB-001
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

    // White-labeled consumer credit enrollment state (per-user; mirrors
    // migration 004_revery_consumers; consumer token AES-256-GCM encrypted,
    // no SSN or full reports ever persisted)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS revery_consumers (
            user_id TEXT PRIMARY KEY,
            revery_consumer_id TEXT,
            consumer_token_enc TEXT,
            identity_status TEXT NOT NULL DEFAULT 'pending',
            service_plan TEXT,
            device_verified_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        (),
    )
    .await?;

    // Verified ID presentation/verification sessions (mirrors migration
    // 005_verified_id_verifications; correlated by `state`, outcome written
    // by the asynchronous Microsoft Request Service callback)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS verified_id_verifications (
            id TEXT PRIMARY KEY,
            state TEXT UNIQUE NOT NULL,
            user_id TEXT,
            request_id TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            presented_claims TEXT,
            subject_did TEXT,
            error TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            expires_at DATETIME
        )",
        (),
    )
    .await?;

    // Plaid-linked items: one row per institution so users can connect
    // multiple banks. The access token is encrypted at rest.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS plaid_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            item_id TEXT NOT NULL UNIQUE,
            access_token TEXT NOT NULL,
            institution_id TEXT NOT NULL,
            institution_name TEXT NOT NULL,
            account_ids TEXT NOT NULL DEFAULT '[]',
            status TEXT NOT NULL DEFAULT 'Active',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        (),
    )
    .await?;
    let _ = conn
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_plaid_items_user_id ON plaid_items(user_id)",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_plaid_items_user_status ON plaid_items(user_id, status)",
            (),
        )
        .await;

    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize core table indexes
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_core_indexes(conn: &Connection) -> Result<()> {
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_user_profile_email ON user_profile(email)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_user_profile_azure_id ON user_profile(azure_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_user_preferences_user ON user_preferences(user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_api_keys_user_email ON api_keys(user_email)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_api_keys_key_prefix ON api_keys(key_prefix)",
    )
    .await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_webauthn_credentials_user_id ON webauthn_credentials(user_id)").await?;
    try_create_index(conn, "CREATE UNIQUE INDEX IF NOT EXISTS idx_webauthn_credentials_credential_id ON webauthn_credentials(credential_id)").await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_kilt_dids_did_uri ON kilt_dids(did_uri)",
    )
    .await?;
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
