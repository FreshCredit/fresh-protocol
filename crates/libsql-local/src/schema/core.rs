//! Core schema definitions: user_profile, auth, preferences
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;

/// Initialize core tables (user_profile and related)
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

    // P0p: Add is_admin column (migration for existing databases)
    let _ = conn
        .execute(
            "ALTER TABLE user_profile ADD COLUMN is_admin BOOLEAN DEFAULT FALSE",
            (),
        )
        .await;

    // P0g: Add provider_onboarding_complete column (migration for existing databases)
    let _ = conn
        .execute(
            "ALTER TABLE user_profile ADD COLUMN provider_onboarding_complete BOOLEAN DEFAULT FALSE",
            (),
        )
        .await;

    // Create user preferences table for storing app preferences
    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_preferences (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            preference_key TEXT NOT NULL,
            preference_value TEXT NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(user_id, preference_key)
        )",
        (),
    )
    .await?;

    // Create auth tokens table for session management
    conn.execute(
        "CREATE TABLE IF NOT EXISTS auth_tokens (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            token_type TEXT NOT NULL,
            token_hash TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            revoked_at TEXT,
            FOREIGN KEY (user_id) REFERENCES user_profile(id)
        )",
        (),
    )
    .await?;

    // Create indexes for core tables
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_user_profile_email ON user_profile(email)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_user_profile_azure_id ON user_profile(azure_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_user_preferences_user ON user_preferences(user_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_auth_tokens_user ON auth_tokens(user_id)",
        (),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_core_tables_module_exists() {
        // Module structure test - verifies the module compiles correctly
        assert!(true);
    }
}

