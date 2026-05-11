use anyhow::Result;
use libsql::Connection;

/// Initialize verification tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_verification_tables(conn: &Connection) -> Result<()> {
    conn.execute(
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

    conn.execute(
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

    conn.execute(
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

    Ok(())
}
