use crate::schema::add_column_if_not_exists;
use anyhow::Result;
use libsql::Connection;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize verification tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_verification_tables(conn: &Connection) -> Result<()> {
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
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
    )
    .await?;

    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
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
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // CANONICAL DEFINITION of `verified_credentials`.
    // This table is also defined in `schema/identity.rs` (which adds
    // `blockchain_hash`/`block_number`); whichever module runs first creates
    // it. This module is the canonical owner: the idempotent ALTERs below
    // converge any existing database (either creation order) to the union of
    // both column sets. Do not change column sets without updating both
    // sites (see schema inventory §6).
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
            revocation_id TEXT,
            credential_data TEXT,
            raw_credential_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;

    // Convergence ALTERs (idempotent — duplicate-column errors are ignored):
    // if `identity.rs` created the table first, add the columns this canonical
    // definition carries that it lacks, and vice versa.
    add_column_if_not_exists(conn, "verified_credentials", "revocation_id", "TEXT").await?;
    add_column_if_not_exists(conn, "verified_credentials", "credential_data", "TEXT").await?;
    add_column_if_not_exists(conn, "verified_credentials", "blockchain_hash", "TEXT").await?;
    add_column_if_not_exists(conn, "verified_credentials", "block_number", "INTEGER").await?;

    Ok(())
}
