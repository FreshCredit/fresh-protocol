//! Versioned database migration system for `FreshCredit`
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

#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use tokio::fs;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Represents a migration to be applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    /// Migration version number
    pub version: i64,
    /// Human-readable migration name
    pub name: String,
    /// Forward migration SQL
    pub up_sql: String,
    /// Rollback migration SQL (optional)
    pub down_sql: Option<String>,
    /// SHA-256 checksum of `up_sql`
    pub checksum: String,
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Represents an applied migration record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedMigration {
    /// Migration version number
    pub version: i64,
    /// Human-readable migration name
    pub name: String,
    /// SHA-256 checksum of the applied migration SQL
    pub checksum: String,
    /// Timestamp when the migration was applied
    pub applied_at: DateTime<Utc>,
}

/// Migration runner for `LibSQL` databases
#[derive(Debug)]
pub struct MigrationRunner {
    connection: libsql::Connection,
    migrations: BTreeMap<i64, Migration>,
}

impl MigrationRunner {
    /// Create a new migration runner with the given connection
    #[must_use]
    pub const fn new(connection: libsql::Connection) -> Self {
        Self {
            connection,
            migrations: BTreeMap::new(),
        }
    }
    // TAG: surface=database owner=platform-team rule=GENERAL-001

    /// Load migrations from a directory
    /// P0-FIX: Now async using `tokio::fs` to avoid blocking I/O
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    /// Load a single migration file and return its parsed metadata and SQL.
    async fn load_single_migration_file(
        path: &std::path::Path,
    ) -> Result<Option<(i64, String, String, bool)>> {
        if path.extension().is_none_or(|e| e != "sql") {
            return Ok(None);
        }
        let filename = path
            .file_stem()
            .ok_or_else(|| anyhow::anyhow!("Invalid filename: {}", path.display()))?
            .to_string_lossy();
        let sql = fs::read_to_string(path).await?;

        if filename.ends_with(".up") {
            let (version, name) = parse_migration_filename(&filename.replace(".up", ""))?;
            Ok(Some((version, name, sql, true)))
        } else if filename.ends_with(".down") {
            let (version, name) = parse_migration_filename(&filename.replace(".down", ""))?;
            Ok(Some((version, name, sql, false)))
        } else {
            Ok(None)
        }
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Loads migrations from a directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read or migration files are invalid.
    pub async fn load_migrations_from_dir(&mut self, dir: &Path) -> Result<()> {
        if !fs::try_exists(dir).await? {
            return Err(anyhow::anyhow!(
                "Migration directory not found: {}",
                dir.display()
            ));
        }

        let mut entries = fs::read_dir(dir).await?;
        let mut up_migrations: BTreeMap<i64, (String, String)> = BTreeMap::new();
        let mut down_migrations: BTreeMap<i64, String> = BTreeMap::new();

        while let Some(entry) = entries.next_entry().await? {
            if let Some((version, name, sql, is_up)) =
                Self::load_single_migration_file(&entry.path()).await?
            {
                if is_up {
                    up_migrations.insert(version, (name, sql));
                } else {
                    down_migrations.insert(version, sql);
                }
            }
        }

        // TAG: surface=database owner=platform-team rule=DB-001
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

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Ensure the `schema_migrations` table exists
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
    ///
    /// Ensures the `schema_migrations` bookkeeping table exists first so callers
    /// (e.g. `check_checksum_mismatches`) can safely run before any migration
    /// has been applied. The ensure step is idempotent (`CREATE TABLE IF NOT EXISTS`).
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_applied_migrations(&self) -> Result<Vec<AppliedMigration>> {
        self.ensure_migrations_table().await?;
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
                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc));
            applied.push(AppliedMigration {
                version,
                name,
                checksum,
                applied_at,
            });
        }
        Ok(applied)
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Run all pending migrations
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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

    /// Apply a single migration with transaction safety
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    async fn apply_migration(&self, migration: &Migration) -> Result<()> {
        // Begin transaction for atomic migration application
        // TAG: surface=database owner=platform-team rule=GENERAL-001
        let tx = self.connection.transaction().await?;

        // Execute the up SQL (may contain multiple statements).
        // Full-line `--` comments are removed BEFORE splitting on `;`:
        // libsql rejects comment-only chunks ("no SQL statement provided")
        // and comments may themselves contain semicolons.
        let up_sql = remove_comment_lines(&migration.up_sql);
        for statement in up_sql.split(';') {
            let statement = statement.trim();
            if !statement.is_empty() {
                Self::execute_statement_idempotent(&tx, statement).await?;
            }
        }

        // Record the migration
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?, ?, ?, ?)",
            libsql::params![
                migration.version,
                migration.name.clone(),
                migration.checksum.clone(),
                Utc::now().to_rfc3339()
            ],
        ).await?;

        // Commit transaction
        tx.commit().await?;

        info!(
            "✅ Applied migration {}: {}",
            migration.version, migration.name
        );
        Ok(())
    }

    /// Execute a single migration statement, making `ALTER TABLE ... ADD COLUMN`
    /// idempotent by skipping the statement when the target column already exists.
    ///
    /// SQLite/libsql do not support `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`,
    /// and migrations that add columns to long-lived tables must be replayable
    /// against databases that already received those columns from an earlier
    /// schema path or manual change.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    async fn execute_statement_idempotent(tx: &libsql::Transaction, statement: &str) -> Result<()> {
        if let Some((table, column)) = parse_alter_add_column(statement) {
            if column_exists(tx, &table, &column).await? {
                info!(
                    "Skipping idempotent ALTER TABLE: column {}.{} already exists",
                    table, column
                );
                return Ok(());
            }
        }
        tx.execute(statement, ()).await?;
        Ok(())
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Rollback a specific migration version with transaction safety
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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

        // Begin transaction for atomic rollback
        let tx = self.connection.transaction().await?;

        // Execute the down SQL (same comment handling as apply_migration)
        let down_sql_clean = remove_comment_lines(down_sql);
        for statement in down_sql_clean.split(';') {
            let statement = statement.trim();
            if !statement.is_empty() {
                tx.execute(statement, ()).await?;
            }
        }

        // Remove the migration record
        tx.execute(
            "DELETE FROM schema_migrations WHERE version = ?",
            libsql::params![version],
        )
        .await?;

        // Commit transaction
        tx.commit().await?;

        info!("⏪ Rolled back migration {}: {}", version, migration.name);
        Ok(())
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Check for checksum mismatches (modified migrations)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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

/// If `statement` is `ALTER TABLE <table> ADD [COLUMN] <column> ...`,
/// return the table and column names. Handles unquoted, double-quoted,
/// backtick-quoted, and bracketed identifiers, and is case-insensitive.
fn parse_alter_add_column(statement: &str) -> Option<(String, String)> {
    let tokens = tokenize_sql(statement);
    if tokens.len() < 5 {
        return None;
    }
    if !tokens[0].eq_ignore_ascii_case("ALTER") || !tokens[1].eq_ignore_ascii_case("TABLE") {
        return None;
    }
    let table = strip_quotes(tokens[2]);
    if !tokens[3].eq_ignore_ascii_case("ADD") {
        return None;
    }
    let column_idx = if tokens
        .get(4)
        .is_some_and(|t| t.eq_ignore_ascii_case("COLUMN"))
    {
        5
    } else {
        4
    };
    let column = strip_quotes(tokens.get(column_idx)?);
    if table.is_empty() || column.is_empty() {
        return None;
    }
    Some((table.to_string(), column.to_string()))
}

/// Tokenize a simple SQL statement, preserving quoted identifiers that may
/// contain whitespace. Used only for `ALTER TABLE ... ADD COLUMN` detection.
fn tokenize_sql(sql: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    let mut quote: Option<char> = None;
    for (i, c) in sql.char_indices() {
        match quote {
            Some(q) => {
                if c == q {
                    tokens.push(&sql[start.unwrap()..=i]);
                    quote = None;
                    start = None;
                }
            }
            None => {
                if c.is_whitespace() {
                    if let Some(s) = start {
                        tokens.push(&sql[s..i]);
                        start = None;
                    }
                } else if c == '"' || c == '\'' || c == '[' {
                    if let Some(s) = start {
                        tokens.push(&sql[s..i]);
                    }
                    quote = Some(c);
                    start = Some(i);
                } else {
                    start.get_or_insert(i);
                }
            }
        }
    }
    if let Some(s) = start {
        tokens.push(&sql[s..]);
    }
    tokens
}

/// Strip optional surrounding quotes from an identifier.
fn strip_quotes(ident: &str) -> &str {
    ident
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| ident.strip_prefix('`').and_then(|s| s.strip_suffix('`')))
        .or_else(|| ident.strip_prefix('[').and_then(|s| s.strip_suffix(']')))
        .unwrap_or(ident)
}

/// Query `PRAGMA table_info(table_name)` to determine whether `column_name`
/// already exists on `table_name`.
async fn column_exists(
    tx: &libsql::Transaction,
    table_name: &str,
    column_name: &str,
) -> Result<bool> {
    let mut rows = tx
        .query(
            "SELECT COUNT(*) FROM pragma_table_info(?) WHERE name = ?",
            libsql::params![table_name, column_name],
        )
        .await?;
    if let Some(row) = rows.next().await? {
        let count: i64 = row.get(0)?;
        return Ok(count > 0);
    }
    Ok(false)
}

/// Parse migration filename to extract version and name
#[allow(clippy::literal_string_with_formatting_args)] // double braces are intentional literal output
fn parse_migration_filename(filename: &str) -> Result<(i64, String)> {
    // TAG: surface=database owner=platform-team rule=DB-001
    let parts: Vec<&str> = filename.splitn(2, '_').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid migration filename format: {filename}. Expected: {{version}}_{{name}}"
        ));
    }
    let version: i64 = parse_migration_version(parts[0])?;
    let name = parts[1].to_string();
    Ok((version, name))
}

/// Parse the numeric version prefix of a migration filename.
fn parse_migration_version(raw: &str) -> Result<i64> {
    raw.parse()
        .with_context(|| format!("Invalid version number: {raw}"))
}

/// Compute SHA-256 checksum of SQL content
fn compute_checksum(sql: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    hex::encode(hasher.finalize())
}

/// Remove full-line `--` comments (and blank lines) from SQL content.
///
/// Done before splitting on `;` because (a) libsql rejects chunks that
/// contain only comments with "no SQL statement provided" and (b) comment
/// text may itself contain semicolons, which would otherwise split a
/// comment into an executable-looking fragment. Only whole lines whose
/// first non-whitespace characters are `--` are removed; SQL lines with
/// trailing inline content are kept intact.
fn remove_comment_lines(sql: &str) -> String {
    sql.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with("--")
        })
        .collect::<Vec<_>>()
        .join("\n")
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

        // TAG: surface=database owner=platform-team rule=DB-001
        // Same SQL should produce same checksum
        let checksum2 = compute_checksum(sql);
        assert_eq!(checksum, checksum2);

        // Different SQL should produce different checksum
        let different_sql = "CREATE TABLE test2 (id INTEGER PRIMARY KEY);";
        let checksum3 = compute_checksum(different_sql);
        assert_ne!(checksum, checksum3);
    }

    #[test]
    fn test_parse_alter_add_column() {
        assert_eq!(
            parse_alter_add_column("ALTER TABLE report_shares ADD COLUMN share_ciphertext TEXT"),
            Some(("report_shares".to_string(), "share_ciphertext".to_string()))
        );
        assert_eq!(
            parse_alter_add_column("ALTER TABLE `report_shares` ADD share_key_wrap TEXT"),
            Some(("report_shares".to_string(), "share_key_wrap".to_string()))
        );
        assert_eq!(
            parse_alter_add_column("ALTER TABLE \"Report Shares\" ADD COLUMN \"shareKey\" TEXT"),
            Some(("Report Shares".to_string(), "shareKey".to_string()))
        );
        // Non-ALTER statements return None.
        assert!(parse_alter_add_column("CREATE TABLE t (id INT)").is_none());
        assert!(parse_alter_add_column("ALTER TABLE t DROP COLUMN c").is_none());
    }
}

#[cfg(test)]
mod async_tests {
    use super::*;

    async fn in_memory_runner() -> Result<MigrationRunner> {
        let db = libsql::Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        Ok(MigrationRunner::new(conn))
    }

    #[tokio::test]
    async fn test_ensure_migrations_table() {
        let runner = in_memory_runner().await.unwrap();
        runner.ensure_migrations_table().await.unwrap();
        // Should be idempotent
        runner.ensure_migrations_table().await.unwrap();
    }

    #[tokio::test]
    async fn test_get_applied_migrations_empty() {
        let runner = in_memory_runner().await.unwrap();
        runner.ensure_migrations_table().await.unwrap();
        let applied = runner.get_applied_migrations().await.unwrap();
        assert!(applied.is_empty());
    }

    #[tokio::test]
    async fn test_load_migrations_from_dir_not_found() {
        let mut runner = in_memory_runner().await.unwrap();
        let result = runner
            .load_migrations_from_dir(Path::new("/nonexistent/migrations"))
            .await;
        assert!(result.is_err());
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    #[tokio::test]
    async fn test_load_migrations_from_dir_success() {
        let temp_dir = std::env::temp_dir().join(format!("test_migrations_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        std::fs::write(
            temp_dir.join("001_initial_schema.up.sql"),
            "CREATE TABLE test1 (id INTEGER PRIMARY KEY);",
        )
        .unwrap();
        std::fs::write(
            temp_dir.join("001_initial_schema.down.sql"),
            "DROP TABLE test1;",
        )
        .unwrap();
        std::fs::write(
            temp_dir.join("002_add_indexes.up.sql"),
            "CREATE INDEX idx_test1 ON test1(id);",
        )
        .unwrap();

        let mut runner = in_memory_runner().await.unwrap();
        runner.load_migrations_from_dir(&temp_dir).await.unwrap();
        assert_eq!(runner.migrations.len(), 2);
        assert!(runner.migrations.contains_key(&1));
        assert!(runner.migrations.contains_key(&2));
        assert!(runner.migrations[&1].down_sql.is_some());
        assert!(runner.migrations[&2].down_sql.is_none());

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    #[tokio::test]
    async fn test_apply_and_get_applied_migration() {
        let runner = in_memory_runner().await.unwrap();
        runner.ensure_migrations_table().await.unwrap();

        let migration = Migration {
            version: 1,
            name: "initial".to_string(),
            up_sql: "CREATE TABLE test_apply (id INTEGER PRIMARY KEY)".to_string(),
            down_sql: Some("DROP TABLE test_apply".to_string()),
            checksum: compute_checksum("CREATE TABLE test_apply (id INTEGER PRIMARY KEY)"),
        };

        runner.apply_migration(&migration).await.unwrap();

        let applied = runner.get_applied_migrations().await.unwrap();
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].version, 1);
        assert_eq!(applied[0].name, "initial");
    }

    #[tokio::test]
    async fn test_apply_migration_idempotent_add_column() {
        let runner = in_memory_runner().await.unwrap();
        runner.ensure_migrations_table().await.unwrap();

        // Create table and add the column once.
        let migration1 = Migration {
            version: 1,
            name: "create_and_add".to_string(),
            up_sql: "CREATE TABLE idempotent_test (id INTEGER PRIMARY KEY); \
                     ALTER TABLE idempotent_test ADD COLUMN extra TEXT"
                .to_string(),
            down_sql: Some("DROP TABLE idempotent_test".to_string()),
            checksum: compute_checksum("ignored"),
        };
        runner.apply_migration(&migration1).await.unwrap();

        // Re-apply the same ADD COLUMN in a later migration; it must be skipped.
        let migration2 = Migration {
            version: 2,
            name: "add_again".to_string(),
            up_sql: "ALTER TABLE idempotent_test ADD COLUMN extra TEXT".to_string(),
            down_sql: None,
            checksum: compute_checksum("ignored2"),
        };
        runner.apply_migration(&migration2).await.unwrap();

        let applied = runner.get_applied_migrations().await.unwrap();
        assert_eq!(applied.len(), 2);
    }

    #[tokio::test]
    async fn test_apply_migration_with_leading_comment_block() {
        // Regression: libsql rejects chunks containing only `--` comments
        // ("no SQL statement provided"). Migration files may start with a
        // comment header; the runner must strip it before executing.
        let mut runner = in_memory_runner().await.unwrap();
        runner.ensure_migrations_table().await.unwrap();

        // Note: comment text deliberately contains a semicolon ("user; it")
        // — the exact pattern that broke migration 002 in production.
        let up = "-- header comment line one\n-- scoped to a single user; it is stored once\n\
                  CREATE TABLE comment_test (id INTEGER PRIMARY KEY);\n\
                  -- trailing comment after final semicolon\n";
        let migration = Migration {
            version: 1,
            name: "commented".to_string(),
            up_sql: up.to_string(),
            down_sql: Some("-- drop it\nDROP TABLE comment_test;".to_string()),
            checksum: compute_checksum(up),
        };

        runner.apply_migration(&migration).await.unwrap();
        let applied = runner.get_applied_migrations().await.unwrap();
        assert_eq!(applied.len(), 1);

        // Rollback path uses the same stripping
        runner.migrations.insert(1, migration);
        runner.rollback_migration(1).await.unwrap();
        let applied = runner.get_applied_migrations().await.unwrap();
        assert!(applied.is_empty());
    }

    #[test]
    fn test_remove_comment_lines() {
        assert_eq!(remove_comment_lines("-- only a comment"), "");
        assert_eq!(remove_comment_lines("\n-- c1\n-- c2\n"), "");
        assert_eq!(
            remove_comment_lines("-- header\nCREATE TABLE t (id INT);"),
            "CREATE TABLE t (id INT);"
        );
        assert_eq!(
            remove_comment_lines("\n\n  -- indented\nSELECT 1"),
            "SELECT 1"
        );
        assert_eq!(remove_comment_lines("SELECT 1"), "SELECT 1");
        // Semicolons inside comment lines must not survive to statement splitting
        let cleaned =
            remove_comment_lines("-- scoped to one user; it is stored\nCREATE TABLE t (id INT);");
        let stmts: Vec<&str> = cleaned
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(stmts, vec!["CREATE TABLE t (id INT)"]);
    }

    #[tokio::test]
    async fn test_run_pending_migrations() {
        let temp_dir = std::env::temp_dir().join(format!("test_pending_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(
            temp_dir.join("001_create_test.up.sql"),
            "CREATE TABLE pending_test (id INTEGER PRIMARY KEY);",
        )
        .unwrap();
        std::fs::write(
            temp_dir.join("002_insert_data.up.sql"),
            "INSERT INTO pending_test VALUES (1);",
        )
        .unwrap();

        let mut runner = in_memory_runner().await.unwrap();
        runner.load_migrations_from_dir(&temp_dir).await.unwrap();
        let applied = runner.run_pending_migrations().await.unwrap();
        assert_eq!(applied.len(), 2);
        assert_eq!(applied, vec![1, 2]);

        // TAG: surface=database owner=platform-team rule=DB-001
        // Running again should apply nothing
        let applied2 = runner.run_pending_migrations().await.unwrap();
        assert!(applied2.is_empty());

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[tokio::test]
    async fn test_rollback_migration() {
        let mut runner = in_memory_runner().await.unwrap();
        runner.ensure_migrations_table().await.unwrap();

        let migration = Migration {
            version: 1,
            name: "create_test".to_string(),
            up_sql: "CREATE TABLE rollback_test (id INTEGER PRIMARY KEY)".to_string(),
            down_sql: Some("DROP TABLE rollback_test".to_string()),
            checksum: compute_checksum("CREATE TABLE rollback_test (id INTEGER PRIMARY KEY)"),
        };

        runner.apply_migration(&migration).await.unwrap();
        let applied = runner.get_applied_migrations().await.unwrap();
        assert_eq!(applied.len(), 1);

        // Insert into runner.migrations so rollback_migration can find it
        runner.migrations.insert(1, migration);
        runner.rollback_migration(1).await.unwrap();

        let applied_after = runner.get_applied_migrations().await.unwrap();
        assert!(applied_after.is_empty());
    }

    #[tokio::test]
    async fn test_check_checksum_mismatches() {
        let runner = in_memory_runner().await.unwrap();
        runner.ensure_migrations_table().await.unwrap();

        // TAG: surface=database owner=platform-team rule=DB-001
        let migration = Migration {
            version: 1,
            name: "initial".to_string(),
            up_sql: "CREATE TABLE checksum_test (id INTEGER PRIMARY KEY)".to_string(),
            down_sql: None,
            checksum: compute_checksum("CREATE TABLE checksum_test (id INTEGER PRIMARY KEY)"),
        };

        runner.apply_migration(&migration).await.unwrap();

        // No mismatch when checksums match
        let mismatches = runner.check_checksum_mismatches().await.unwrap();
        assert!(mismatches.is_empty());
    }

    #[tokio::test]
    async fn test_check_checksum_mismatches_mismatch() {
        let mut runner = in_memory_runner().await.unwrap();
        runner.ensure_migrations_table().await.unwrap();

        let migration = Migration {
            version: 1,
            name: "initial".to_string(),
            up_sql: "CREATE TABLE checksum_test (id INTEGER PRIMARY KEY)".to_string(),
            down_sql: None,
            checksum: compute_checksum("CREATE TABLE checksum_test (id INTEGER PRIMARY KEY)"),
        };

        runner.apply_migration(&migration).await.unwrap();

        // Insert a migration with a different checksum into self.migrations
        let tampered_migration = Migration {
            version: 1,
            name: "initial".to_string(),
            up_sql: "CREATE TABLE checksum_test (id INTEGER PRIMARY KEY)".to_string(),
            down_sql: None,
            checksum: compute_checksum("TAMPERED SQL"),
        };
        runner.migrations.insert(1, tampered_migration);

        // TAG: surface=database owner=platform-team rule=DB-001
        let mismatches = runner.check_checksum_mismatches().await.unwrap();
        assert_eq!(mismatches.len(), 1);
        assert_eq!(mismatches[0].0, 1);
    }

    #[tokio::test]
    async fn test_rollback_migration_not_found() {
        let runner = in_memory_runner().await.unwrap();
        let result = runner.rollback_migration(999).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rollback_migration_no_down_sql() {
        let mut runner = in_memory_runner().await.unwrap();
        runner.ensure_migrations_table().await.unwrap();

        let migration = Migration {
            version: 1,
            name: "no_down".to_string(),
            up_sql: "CREATE TABLE no_down_test (id INTEGER PRIMARY KEY)".to_string(),
            down_sql: None,
            checksum: compute_checksum("CREATE TABLE no_down_test (id INTEGER PRIMARY KEY)"),
        };

        runner.apply_migration(&migration).await.unwrap();
        runner.migrations.insert(1, migration);

        let result = runner.rollback_migration(1).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_migration_filename_invalid() {
        assert!(parse_migration_filename("invalid").is_err());
        assert!(parse_migration_filename("").is_err());
        assert!(parse_migration_filename("noversion").is_err());
    }

    #[test]
    fn test_parse_migration_filename_bad_version() {
        assert!(parse_migration_filename("abc_name").is_err());
    }
}
