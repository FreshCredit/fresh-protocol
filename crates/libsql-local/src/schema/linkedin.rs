//! `LinkedIn` professional data schema definitions
//!
//! Contains `LinkedIn` tables:
//! - `linkedin_profiles`: Profile data
//! - `linkedin_experiences`: Work experience
//! - `linkedin_education`: Education history
//! - `linkedin_skills`: Skills and endorsements
//! - `linkedin_certifications`: Professional certifications
//! - `linkedin_languages`: Language proficiencies
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;
use tracing::info;

/// Initialize `LinkedIn` tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_linkedin_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing linkedin tables");
    conn.execute(
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

    conn.execute(
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

    conn.execute(
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

    conn.execute(
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

    conn.execute(
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

    conn.execute(
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

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_linkedin_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}
