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

use super::{add_column_if_not_exists, try_create_index};
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize core tables (`user_profile` and related)
///
/// Creates 6 core tables: `user_profile`, `user_preferences`, `auth_tokens`,
/// `api_keys`, `webauthn_credentials`, `kilt_dids`
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_core_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing core tables");
    create_user_profile_table(conn).await?;
    create_user_identities_table(conn).await?;
    migrate_user_profile_columns(conn).await?;
    create_user_preferences_schema(conn).await?;
    create_api_keys_table(conn).await?;
    migrate_api_keys_columns(conn).await?;
    create_webauthn_and_kilt_schema(conn).await?;
    create_integration_tables(conn).await?;
    create_vault_sync_tables(conn).await?;
    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Creates the `user_profile` table.
async fn create_user_profile_table(conn: &Connection) -> Result<()> {
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
            consumer_verified_id_credential_id TEXT,
            consumer_verified_id_status TEXT DEFAULT 'pending',
            consumer_verified_id_issued_at TEXT,
            provider_verified_id_credential_id TEXT,
            provider_verified_id_status TEXT DEFAULT 'pending',
            provider_verified_id_issued_at TEXT,
            last_report_date DATETIME,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;
    Ok(())
}

/// Creates `user_identities` (linked sign-in identities), its indexes, and
/// the `is_primary` backfill migration.
async fn create_user_identities_table(conn: &Connection) -> Result<()> {
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
    add_column_if_not_exists(
        conn,
        "user_identities",
        "is_primary",
        "BOOLEAN DEFAULT FALSE",
    )
    .await?;
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
    Ok(())
}

/// Idempotent column migrations for existing `user_profile` databases.
async fn migrate_user_profile_columns(conn: &Connection) -> Result<()> {
    // Migrations for existing databases
    // CHATBOT-FIX: platform_user_id is required by RBAC middleware queries
    // This column was added to the schema but existing databases may not have it
    add_column_if_not_exists(conn, "user_profile", "platform_user_id", "TEXT").await?;
    // For rows with NULL platform_user_id, populate from email as fallback
    let _ = conn.execute(
        "UPDATE user_profile SET platform_user_id = email WHERE platform_user_id IS NULL OR platform_user_id = ''",
        (),
    ).await;

    add_column_if_not_exists(conn, "user_profile", "is_admin", "BOOLEAN DEFAULT FALSE").await?;
    add_column_if_not_exists(
        conn,
        "user_profile",
        "provider_onboarding_complete",
        "BOOLEAN DEFAULT FALSE",
    )
    .await?;
    // ARCH-P2-001: Migration for new profile fields
    add_column_if_not_exists(conn, "user_profile", "phone_number", "TEXT").await?;
    add_column_if_not_exists(conn, "user_profile", "preferred_name", "TEXT").await?;
    add_column_if_not_exists(conn, "user_profile", "emergency_contact_name", "TEXT").await?;
    add_column_if_not_exists(conn, "user_profile", "emergency_contact_phone", "TEXT").await?;
    add_column_if_not_exists(conn, "user_profile", "employer_name", "TEXT").await?;

    // Per-side Verified ID issuance tracking: a single user may hold one
    // consumer credential and one provider credential, issued independently.
    // NOTE: these columns are also in the CREATE TABLE above (fresh DBs);
    // these ALTERs cover databases created before they were added.
    // Aligns the local per-user schema with the shared/cloud DDL
    // (schema_manager impls/core.rs, libsql/cloud), which already carries
    // the role-scoped verified-ID columns.
    add_column_if_not_exists(
        conn,
        "user_profile",
        "consumer_verified_id_credential_id",
        "TEXT",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_profile",
        "consumer_verified_id_status",
        "TEXT DEFAULT 'pending'",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_profile",
        "consumer_verified_id_issued_at",
        "TEXT",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_profile",
        "provider_verified_id_credential_id",
        "TEXT",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_profile",
        "provider_verified_id_status",
        "TEXT DEFAULT 'pending'",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_profile",
        "provider_verified_id_issued_at",
        "TEXT",
    )
    .await?;

    Ok(())
}

/// Creates `user_preferences` plus its migrations.
async fn create_user_preferences_schema(conn: &Connection) -> Result<()> {
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
            vault_key_acknowledged BOOLEAN DEFAULT FALSE,
            backup_sync_chosen BOOLEAN DEFAULT FALSE,
            assistant_data_consent BOOLEAN DEFAULT FALSE,
            assistant_model TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // Migrations for user_preferences
    add_column_if_not_exists(conn, "user_preferences", "ai_mode", "TEXT DEFAULT 'auto'").await?;
    add_column_if_not_exists(
        conn,
        "user_preferences",
        "mock_data_enabled",
        "BOOLEAN DEFAULT FALSE",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_preferences",
        "onboarding_completed",
        "BOOLEAN DEFAULT FALSE",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_preferences",
        "onboarding_permanently_dismissed",
        "BOOLEAN DEFAULT FALSE",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_preferences",
        "onboarding_reminder_dismissed_until",
        "DATETIME",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_preferences",
        "plaid_connection_skipped",
        "BOOLEAN DEFAULT FALSE",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_preferences",
        "plaid_reminder_dismissed_until",
        "DATETIME",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_preferences",
        "vault_key_acknowledged",
        "BOOLEAN DEFAULT FALSE",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_preferences",
        "backup_sync_chosen",
        "BOOLEAN DEFAULT FALSE",
    )
    .await?;
    add_column_if_not_exists(
        conn,
        "user_preferences",
        "assistant_data_consent",
        "BOOLEAN DEFAULT FALSE",
    )
    .await?;
    add_column_if_not_exists(conn, "user_preferences", "assistant_model", "TEXT").await?;

    Ok(())
}

/// Creates `api_keys` and rebuilds tables created with the legacy
/// (`user_id`/`key_name`) schema.
async fn create_api_keys_table(conn: &Connection) -> Result<()> {
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
            add_column_if_not_exists(conn, "api_keys_old", "permissions", "TEXT DEFAULT 'read'")
                .await?;
            add_column_if_not_exists(conn, "api_keys_old", "is_revoked", "BOOLEAN DEFAULT FALSE")
                .await?;
            add_column_if_not_exists(conn, "api_keys_old", "last_used_at", "DATETIME").await?;
            add_column_if_not_exists(conn, "api_keys_old", "expires_at", "DATETIME").await?;
            add_column_if_not_exists(conn, "api_keys_old", "updated_at", "DATETIME").await?;
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
    Ok(())
}

/// Idempotent column migrations bringing `api_keys` up to the union schema.
async fn migrate_api_keys_columns(conn: &Connection) -> Result<()> {
    // Ensure all columns exist for tables that may have been created with an
    // older version of the correct schema.
    add_column_if_not_exists(conn, "api_keys", "is_revoked", "BOOLEAN DEFAULT FALSE").await?;
    add_column_if_not_exists(conn, "api_keys", "last_used_at", "DATETIME").await?;
    add_column_if_not_exists(conn, "api_keys", "expires_at", "DATETIME").await?;
    add_column_if_not_exists(
        conn,
        "api_keys",
        "updated_at",
        "DATETIME DEFAULT CURRENT_TIMESTAMP",
    )
    .await?;
    add_column_if_not_exists(conn, "api_keys", "revoked_at", "DATETIME").await?;
    add_column_if_not_exists(conn, "api_keys", "revoked_reason", "TEXT").await?;
    add_column_if_not_exists(conn, "api_keys", "rotated_at", "DATETIME").await?;
    // Credential-vault columns for retrievable service credentials
    // (encrypted_value holds AES-256-GCM ciphertext; NULL for legacy
    // hash-only API keys).
    add_column_if_not_exists(conn, "api_keys", "encrypted_value", "TEXT").await?;
    add_column_if_not_exists(conn, "api_keys", "service", "TEXT").await?;
    add_column_if_not_exists(conn, "api_keys", "environment", "TEXT").await?;
    // Union-schema columns from the shared (production) api_keys shape.
    // The adapter writes the union column set so the same INSERT works on
    // both schemas; these idempotent ALTERs bring the local table up to the
    // union without a table rebuild.
    add_column_if_not_exists(conn, "api_keys", "user_id", "TEXT").await?;
    add_column_if_not_exists(conn, "api_keys", "key_name", "TEXT").await?;
    add_column_if_not_exists(conn, "api_keys", "rate_limit", "INTEGER DEFAULT 1000").await?;
    add_column_if_not_exists(conn, "api_keys", "is_active", "BOOLEAN DEFAULT TRUE").await?;

    Ok(())
}

/// Creates `webauthn_credentials` and `kilt_dids`.
async fn create_webauthn_and_kilt_schema(conn: &Connection) -> Result<()> {
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

    Ok(())
}

/// Creates integration tables (`revery_consumers`, `verified_id_verifications`,
/// Creates integration tables (`revery_consumers`, `verified_id_verifications`,
/// `plaid_items`, `user_connections`).
async fn create_integration_tables(conn: &Connection) -> Result<()> {
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

    // V2 consumer connection toggles (per-user app connect/disconnect state).
    // Mirrors the shared_db table of the same name (schema_manager
    // impls/core.rs): the server upserts it on every connect/disconnect and
    // mirrors each write into the per-user vault (vault-as-source-of-truth,
    // phase 2), from which the browser syncs.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_connections (
            user_id TEXT NOT NULL,
            connection_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'disconnected',
            connected_at TEXT,
            PRIMARY KEY (user_id, connection_id)
        )",
        (),
    )
    .await?;
    Ok(())
}

/// Creates the vault-backed `approved_data` and `sync_deletions` tables.
async fn create_vault_sync_tables(conn: &Connection) -> Result<()> {
    // V2 consumer approved data items. Mirrors the shared_db `approved_data`
    // table (schema_manager impls/core.rs): finalized approved staged rows
    // are written here so the browser vault sync can read them locally and
    // report generation can count them as vault-backed data.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS approved_data (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            category TEXT NOT NULL,
            label TEXT NOT NULL,
            approved_at TEXT NOT NULL,
            hash_prefix TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;
    let _ = conn
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_approved_data_user_id ON approved_data(user_id)",
            (),
        )
        .await;

    // Deletion propagation ledger (slice C1): every hard delete records a
    // tombstone here in the same transaction, and the browser HTTP sync
    // replays it in both directions. Mirrors the browser OPFS schema in
    // apps/app/static/js/libsql-browser-opfs.js. No user_id column: this is
    // a per-user database.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_deletions (
            id TEXT PRIMARY KEY,
            table_name TEXT NOT NULL,
            row_id TEXT NOT NULL,
            deleted_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(table_name, row_id)
        )",
        (),
    )
    .await?;
    let _ = conn
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_sync_deletions_deleted_at ON sync_deletions(deleted_at)",
            (),
        )
        .await;

    Ok(())
}

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
