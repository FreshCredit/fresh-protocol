//! Versioned database migration system for FreshCredit
//!
//! This module provides a migration runner that:
//! 1. Tracks applied migrations in a `schema_migrations` table
//! 2. Applies migrations in order by version number
//! 3. Generates checksums to detect modified migrations
//! 4. Supports rollback via down migrations (when provided)
//!
//! Migration files are stored in `migrations/` directory with format:
//! - `{version}_{name}.up.sql` - Forward migration
//! - `{version}_{name}.down.sql` - Rollback migration (optional)

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use tokio::fs;
use tracing::info;

/// Represents a migration to be applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    pub version: i64,
    pub name: String,
    pub up_sql: String,
    pub down_sql: Option<String>,
    pub checksum: String,
}

/// Represents an applied migration record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedMigration {
    pub version: i64,
    pub name: String,
    pub checksum: String,
    pub applied_at: DateTime<Utc>,
}

/// Migration runner for LibSQL databases
pub struct MigrationRunner {
    connection: libsql::Connection,
    migrations: BTreeMap<i64, Migration>,
}

impl MigrationRunner {
    /// Create a new migration runner with the given connection
    pub fn new(connection: libsql::Connection) -> Self {
        Self {
            connection,
            migrations: BTreeMap::new(),
        }
    }

    /// Load migrations from a directory
    /// P0-FIX: Now async using tokio::fs to avoid blocking I/O
    pub async fn load_migrations_from_dir(&mut self, dir: &Path) -> Result<()> {
        if !fs::try_exists(dir).await? {
            return Err(anyhow::anyhow!("Migration directory not found: {dir:?}"));
        }

        let mut entries = fs::read_dir(dir).await?;
        let mut up_migrations: BTreeMap<i64, (String, String)> = BTreeMap::new();
        let mut down_migrations: BTreeMap<i64, String> = BTreeMap::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "sql").unwrap_or(false) {
                let filename = path
                    .file_stem()
                    .ok_or_else(|| anyhow::anyhow!("Invalid filename: {path:?}"))?
                    .to_string_lossy();
                if filename.ends_with(".up") {
                    let (version, name) = parse_migration_filename(&filename.replace(".up", ""))?;
                    let sql = fs::read_to_string(&path).await?;
                    up_migrations.insert(version, (name, sql));
                } else if filename.ends_with(".down") {
                    let (version, _) = parse_migration_filename(&filename.replace(".down", ""))?;
                    let sql = fs::read_to_string(&path).await?;
                    down_migrations.insert(version, sql);
                }
            }
        }

        for (version, (name, up_sql)) in up_migrations {
            let checksum = compute_checksum(&up_sql);
            let down_sql = down_migrations.get(&version).cloned();
            self.migrations.insert(
                version,
                Migration {
                    version,
                    name,
                    up_sql,
                    down_sql,
                    checksum,
                },
            );
        }
        info!("Loaded {} migrations from {:?}", self.migrations.len(), dir);
        Ok(())
    }

    /// Ensure the schema_migrations table exists
    pub async fn ensure_migrations_table(&self) -> Result<()> {
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                checksum TEXT NOT NULL,
                applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                (),
            )
            .await?;
        Ok(())
    }

    /// Get list of applied migrations
    pub async fn get_applied_migrations(&self) -> Result<Vec<AppliedMigration>> {
        let mut applied = Vec::new();
        let mut rows = self.connection
            .query("SELECT version, name, checksum, applied_at FROM schema_migrations ORDER BY version", ())
            .await?;

        while let Some(row) = rows.next().await? {
            let version: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let checksum: String = row.get(2)?;
            let applied_at_str: String = row.get(3)?;
            let applied_at = DateTime::parse_from_rfc3339(&applied_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            applied.push(AppliedMigration {
                version,
                name,
                checksum,
                applied_at,
            });
        }
        Ok(applied)
    }

    /// Run all pending migrations
    pub async fn run_pending_migrations(&self) -> Result<Vec<i64>> {
        self.ensure_migrations_table().await?;
        let applied = self.get_applied_migrations().await?;
        let applied_versions: std::collections::HashSet<i64> =
            applied.iter().map(|m| m.version).collect();
        let mut applied_now = Vec::new();

        for (version, migration) in &self.migrations {
            if !applied_versions.contains(version) {
                info!("Applying migration {}: {}", version, migration.name);
                self.apply_migration(migration)
                    .await
                    .with_context(|| format!("Failed to apply migration {version}"))?;
                applied_now.push(*version);
            }
        }
        Ok(applied_now)
    }

    /// Apply a single migration
    async fn apply_migration(&self, migration: &Migration) -> Result<()> {
        // Execute the up SQL (may contain multiple statements)
        for statement in migration.up_sql.split(';') {
            let statement = statement.trim();
            if !statement.is_empty() {
                self.connection.execute(statement, ()).await?;
            }
        }

        // Record the migration
        self.connection
            .execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?, ?, ?, ?)",
                libsql::params![
                    migration.version,
                    migration.name.clone(),
                    migration.checksum.clone(),
                    Utc::now().to_rfc3339()
                ],
            )
            .await?;

        info!(
            "✅ Applied migration {}: {}",
            migration.version, migration.name
        );
        Ok(())
    }

    /// Rollback a specific migration version
    pub async fn rollback_migration(&self, version: i64) -> Result<()> {
        let migration = self
            .migrations
            .get(&version)
            .ok_or_else(|| anyhow::anyhow!("Migration {version} not found"))?;

        let down_sql = migration
            .down_sql
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No down migration for version {version}"))?;

        info!("Rolling back migration {}: {}", version, migration.name);

        // Execute the down SQL
        for statement in down_sql.split(';') {
            let statement = statement.trim();
            if !statement.is_empty() {
                self.connection.execute(statement, ()).await?;
            }
        }

        // Remove the migration record
        self.connection
            .execute(
                "DELETE FROM schema_migrations WHERE version = ?",
                libsql::params![version],
            )
            .await?;

        info!("⏪ Rolled back migration {}: {}", version, migration.name);
        Ok(())
    }

    /// Check for checksum mismatches (modified migrations)
    pub async fn check_checksum_mismatches(&self) -> Result<Vec<(i64, String, String)>> {
        let applied = self.get_applied_migrations().await?;
        let mut mismatches = Vec::new();

        for applied_mig in &applied {
            if let Some(migration) = self.migrations.get(&applied_mig.version) {
                if migration.checksum != applied_mig.checksum {
                    mismatches.push((
                        applied_mig.version,
                        applied_mig.checksum.clone(),
                        migration.checksum.clone(),
                    ));
                }
            }
        }

        Ok(mismatches)
    }
}

/// Parse migration filename to extract version and name
fn parse_migration_filename(filename: &str) -> Result<(i64, String)> {
    let parts: Vec<&str> = filename.splitn(2, '_').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid migration filename format: {filename}. Expected: {{version}}_{{name}}"
        ));
    }
    let version: i64 = parts[0]
        .parse()
        .with_context(|| format!("Invalid version number: {}", parts[0]))?;
    let name = parts[1].to_string();
    Ok((version, name))
}

/// Compute SHA-256 checksum of SQL content
fn compute_checksum(sql: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_migration_filename() {
        let (version, name) = parse_migration_filename("001_initial_schema").unwrap();
        assert_eq!(version, 1);
        assert_eq!(name, "initial_schema");

        let (version2, name2) = parse_migration_filename("002_add_indexes").unwrap();
        assert_eq!(version2, 2);
        assert_eq!(name2, "add_indexes");
    }

    #[test]
    fn test_compute_checksum() {
        let sql = "CREATE TABLE test (id INTEGER PRIMARY KEY);";
        let checksum = compute_checksum(sql);
        assert_eq!(checksum.len(), 64); // SHA-256 hex is 64 chars

        // Same SQL should produce same checksum
        let checksum2 = compute_checksum(sql);
        assert_eq!(checksum, checksum2);

        // Different SQL should produce different checksum
        let different_sql = "CREATE TABLE test2 (id INTEGER PRIMARY KEY);";
        let checksum3 = compute_checksum(different_sql);
        assert_ne!(checksum, checksum3);
    }
}
