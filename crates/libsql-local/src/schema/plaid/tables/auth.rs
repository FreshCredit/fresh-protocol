use anyhow::Result;
use libsql::Connection;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_plaid_auth_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing plaid tables");
    // Create auth table for account authentication data
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
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
    )
    .await?;

    // Create identities table for Plaid Identity data per account
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS identities (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            account_holder_names TEXT,
            account_holder_emails TEXT,
            account_holder_phones TEXT,
            account_holder_addresses TEXT,
            primary_email TEXT,
            secondary_email TEXT,
            other_email TEXT,
            primary_phone TEXT,
            home_phone TEXT,
            work_phone TEXT,
            mobile_phone TEXT,
            primary_address_street TEXT,
            primary_address_city TEXT,
            primary_address_region TEXT,
            primary_address_postal_code TEXT,
            primary_address_country TEXT,
            secondary_address_street TEXT,
            secondary_address_city TEXT,
            secondary_address_region TEXT,
            secondary_address_postal_code TEXT,
            secondary_address_country TEXT,
            is_primary_account_holder BOOLEAN DEFAULT FALSE,
            account_holder_type TEXT DEFAULT 'owner',
            raw_identity_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;

    Ok(())
}
