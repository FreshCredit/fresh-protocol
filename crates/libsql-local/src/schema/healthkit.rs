//! HealthKit schema definitions
//!
//! Contains Apple Health tables:
//! - healthkit_profiles: User health profile metadata
//! - healthkit_records: Health records (steps, heart rate, etc.)
//! - healthkit_workouts: Workout sessions
//! - healthkit_activity_summaries: Daily activity rings
//! - healthkit_clinical_records: Clinical health records
//! - healthkit_correlations: Correlated health data
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;
use tracing::info;

/// Initialize HealthKit tables
pub async fn initialize_healthkit_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing healthkit tables");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS healthkit_profiles (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL UNIQUE,
            export_date DATETIME,
            date_of_birth TEXT,
            biological_sex TEXT,
            blood_type TEXT,
            fitzpatrick_skin_type TEXT,
            wheelchair_use TEXT,
            raw_profile_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS healthkit_records (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            healthkit_profile_id TEXT NOT NULL,
            record_type TEXT NOT NULL,
            source_name TEXT,
            source_version TEXT,
            device TEXT,
            unit TEXT,
            value REAL,
            start_date DATETIME NOT NULL,
            end_date DATETIME NOT NULL,
            creation_date DATETIME,
            metadata TEXT,
            raw_record_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (healthkit_profile_id) REFERENCES healthkit_profiles (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS healthkit_workouts (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            healthkit_profile_id TEXT NOT NULL,
            workout_activity_type TEXT NOT NULL,
            duration REAL,
            duration_unit TEXT,
            total_distance REAL,
            distance_unit TEXT,
            total_energy_burned REAL,
            energy_unit TEXT,
            source_name TEXT,
            source_version TEXT,
            device TEXT,
            start_date DATETIME NOT NULL,
            end_date DATETIME NOT NULL,
            creation_date DATETIME,
            metadata TEXT,
            raw_workout_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (healthkit_profile_id) REFERENCES healthkit_profiles (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS healthkit_activity_summaries (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            healthkit_profile_id TEXT NOT NULL,
            date_components TEXT NOT NULL,
            active_energy_burned REAL,
            active_energy_burned_goal REAL,
            active_energy_burned_unit TEXT,
            apple_move_time REAL,
            apple_move_time_goal REAL,
            apple_exercise_time REAL,
            apple_exercise_time_goal REAL,
            apple_stand_hours REAL,
            apple_stand_hours_goal REAL,
            raw_summary_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (healthkit_profile_id) REFERENCES healthkit_profiles (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS healthkit_clinical_records (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            healthkit_profile_id TEXT NOT NULL,
            clinical_type TEXT NOT NULL,
            identifier TEXT,
            source_name TEXT,
            source_url TEXT,
            fhir_resource_type TEXT,
            fhir_resource_data TEXT,
            start_date DATETIME,
            end_date DATETIME,
            raw_clinical_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (healthkit_profile_id) REFERENCES healthkit_profiles (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS healthkit_correlations (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            healthkit_profile_id TEXT NOT NULL,
            correlation_type TEXT NOT NULL,
            source_name TEXT,
            start_date DATETIME NOT NULL,
            end_date DATETIME NOT NULL,
            objects TEXT,
            metadata TEXT,
            raw_correlation_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (healthkit_profile_id) REFERENCES healthkit_profiles (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_healthkit_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}

