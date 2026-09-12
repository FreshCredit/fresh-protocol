//! Identity schema definitions: `identity_verification`, `verified_credentials`
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! - `identity_verification` (singular) is used for Plaid IDV (NOT `identity_verifications` plural)
//! - `verified_credentials` stores Entra Verified ID / KILT DID credentials

use anyhow::Result;
use libsql::Connection;

use super::try_create_index;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize identity-related tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_identity_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing identity tables");
    // Create identity_verification table for Plaid IDV (singular)
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
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
            date_of_birth TEXT,
            id_number TEXT,
            documentary_verification TEXT,
            selfie_check TEXT,
            risk_check TEXT,
            watchlist_screening TEXT,
            raw_identity_verification_data TEXT NOT NULL,
            blockchain_hash TEXT,
            block_number INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;

    // Create verified_credentials table for Entra Verified ID / KILT DID credentials
    // NOTE: `verified_credentials` is defined here AND in
    // `schema/reports/verification.rs`; whichever runs first creates it.
    // The canonical definition lives in `reports/verification.rs`, whose
    // idempotent ALTERs converge either creation order to the union of both
    // column sets (this site's extras: `blockchain_hash`, `block_number`;
    // that site's extras: `revocation_id`, `credential_data`).
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
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
            raw_credential_data TEXT NOT NULL,
            blockchain_hash TEXT,
            block_number INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // Create indexes for identity tables (using defensive helper for cloud schema compatibility)
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_identity_verification_user_id ON identity_verification(user_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_identity_verification_status ON identity_verification(status)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_verified_credentials_user_id ON verified_credentials(user_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_verified_credentials_did_uri ON verified_credentials(did_uri)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_verified_credentials_type ON verified_credentials(credential_type)").await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_identity_tables_module_exists() {
        // Module structure test - verifies the module compiles correctly
        let _ = 1 + 1; // Compile-time verification
    }
}
