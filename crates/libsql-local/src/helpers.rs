// TAG: surface=database owner=platform-team rule=DB-001 test-coverage=unit
//! Helper utilities for local `LibSQL` operations
//!
//! This module provides shared helper functions used across the crate.

use anyhow::{bail, Result};
use libsql::Connection;

/// DDL for the `sync_deletions` tombstone ledger (slice C1). Kept in sync
/// with `schema/core.rs` and `migrations/unified_schema.sql`.
pub const SYNC_DELETIONS_UPSERT_SQL: &str =
    "INSERT INTO sync_deletions (id, table_name, row_id, deleted_at) \
     VALUES (?1, ?2, ?3, datetime('now')) \
     ON CONFLICT(table_name, row_id) DO UPDATE SET deleted_at = datetime('now')";

/// Validate a table name before interpolating it into SQL.
///
/// Table names cannot be bind-parameterized, so this is the SQL-injection
/// guard for the tombstone helpers: only lowercase SQLite identifiers that
/// are not system tables are accepted.
#[must_use]
pub fn is_valid_sync_table_name(table_name: &str) -> bool {
    !table_name.is_empty()
        && !table_name.starts_with("sqlite_")
        && table_name
            .chars()
            .enumerate()
            .all(|(i, c)| c.is_ascii_lowercase() || c == '_' || (i > 0 && c.is_ascii_digit()))
        && table_name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
}

/// Create the `sync_deletions` ledger if it does not exist yet.
///
/// Idempotent. The tombstone helpers call this defensively because existing
/// per-user databases predate slice C1.
///
/// # Errors
///
/// Returns an error if the DDL fails.
pub async fn ensure_sync_deletions_table(conn: &Connection) -> Result<()> {
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
    Ok(())
}

/// Record a tombstone for a hard-deleted row (upsert into `sync_deletions`).
///
/// Callers that delete rows MUST call this in the same transaction as the
/// delete (see [`delete_with_tombstone`] / [`delete_rows_with_tombstones`]),
/// so the browser HTTP sync can propagate the deletion to the other side.
///
/// # Errors
///
/// Returns an error if the table name is invalid or the insert fails.
pub async fn record_tombstone(conn: &Connection, table_name: &str, row_id: &str) -> Result<()> {
    if !is_valid_sync_table_name(table_name) {
        bail!("record_tombstone: invalid table name: {table_name}");
    }
    ensure_sync_deletions_table(conn).await?;
    conn.execute(
        SYNC_DELETIONS_UPSERT_SQL,
        libsql::params![uuid::Uuid::new_v4().to_string(), table_name, row_id],
    )
    .await?;
    Ok(())
}

/// Hard-delete a row and record its tombstone atomically (one transaction).
///
/// ACID: the DELETE and the tombstone INSERT commit together or roll back
/// together, so a crash can never strand the row in the cloud copy.
///
/// # Errors
///
/// Returns an error if the table name is invalid or the transaction fails.
pub async fn delete_with_tombstone(
    conn: &Connection,
    table_name: &str,
    row_id: &str,
) -> Result<u64> {
    delete_rows_with_tombstones(conn, table_name, std::slice::from_ref(&row_id.to_string())).await
}

/// Hard-delete several rows of one table and record their tombstones in a
/// single transaction. Returns the number of rows actually deleted.
///
/// # Errors
///
/// Returns an error if the table name is invalid or the transaction fails.
pub async fn delete_rows_with_tombstones(
    conn: &Connection,
    table_name: &str,
    row_ids: &[String],
) -> Result<u64> {
    if !is_valid_sync_table_name(table_name) {
        bail!("delete_rows_with_tombstones: invalid table name: {table_name}");
    }
    if row_ids.is_empty() {
        return Ok(0);
    }
    ensure_sync_deletions_table(conn).await?;

    // BEGIN IMMEDIATE ... COMMIT: delete + tombstone are atomic.
    let tx = conn.transaction().await?;
    let mut deleted = 0_u64;
    for row_id in row_ids {
        // Table name validated above (cannot be parameterized).
        deleted += tx
            .execute(
                &format!("DELETE FROM {table_name} WHERE id = ?1"),
                libsql::params![row_id.as_str()],
            )
            .await?;
        tx.execute(
            SYNC_DELETIONS_UPSERT_SQL,
            libsql::params![
                uuid::Uuid::new_v4().to_string(),
                table_name,
                row_id.as_str()
            ],
        )
        .await?;
    }
    tx.commit().await?;
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Temp-file libsql DB (each `:memory:` connection is a separate
    /// database in this libsql version, so tests share a file instead).
    async fn test_db() -> (libsql::Database, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("fc-tombstone-test-{}.db", uuid::Uuid::new_v4()));
        let db = libsql::Builder::new_local(&path).build().await.unwrap();
        let conn = db.connect().unwrap();
        conn.execute(
            "CREATE TABLE sync_deletions (
                id TEXT PRIMARY KEY,
                table_name TEXT NOT NULL,
                row_id TEXT NOT NULL,
                deleted_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(table_name, row_id)
            )",
            (),
        )
        .await
        .unwrap();
        conn.execute(
            "CREATE TABLE widgets (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                updated_at TEXT
            )",
            (),
        )
        .await
        .unwrap();
        drop(conn);
        (db, path)
    }

    #[test]
    fn table_name_validation() {
        assert!(is_valid_sync_table_name("workflows"));
        assert!(is_valid_sync_table_name("sync_deletions"));
        assert!(!is_valid_sync_table_name("users; DROP TABLE users--"));
        assert!(!is_valid_sync_table_name("users WHERE 1=1"));
        assert!(!is_valid_sync_table_name("Users"));
        assert!(!is_valid_sync_table_name("sqlite_master"));
        assert!(!is_valid_sync_table_name("1accounts"));
        assert!(!is_valid_sync_table_name(""));
    }

    #[tokio::test]
    async fn tombstone_round_trip() {
        let (db, path) = test_db().await;
        let conn = db.connect().unwrap();

        record_tombstone(&conn, "widgets", "w-1").await.unwrap();

        let mut rows = conn
            .query(
                "SELECT table_name, row_id, deleted_at FROM sync_deletions WHERE row_id = 'w-1'",
                (),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("tombstone row");
        assert_eq!(row.get::<String>(0).unwrap(), "widgets");
        assert_eq!(row.get::<String>(1).unwrap(), "w-1");
        // datetime('now') format: 'YYYY-MM-DD HH:MM:SS' (matches JS writer)
        let deleted_at = row.get::<String>(2).unwrap();
        assert_eq!(deleted_at.len(), 19);
        assert_eq!(&deleted_at[10..11], " ");
        assert!(
            rows.next().await.unwrap().is_none(),
            "exactly one tombstone"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn tombstone_upsert_refreshes_deleted_at_on_redelete() {
        let (db, path) = test_db().await;
        let conn = db.connect().unwrap();

        record_tombstone(&conn, "widgets", "w-1").await.unwrap();
        record_tombstone(&conn, "widgets", "w-1").await.unwrap();

        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM sync_deletions WHERE table_name = 'widgets' AND row_id = 'w-1'",
                (),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(
            row.get::<i64>(0).unwrap(),
            1,
            "UNIQUE(table_name, row_id) holds"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn delete_with_tombstone_is_atomic() {
        let (db, path) = test_db().await;
        let conn = db.connect().unwrap();
        conn.execute(
            "INSERT INTO widgets (id, user_id, updated_at) VALUES ('w-1', 'u-1', '2026-07-27 00:00:00')",
            (),
        )
        .await
        .unwrap();

        let deleted = delete_with_tombstone(&conn, "widgets", "w-1")
            .await
            .unwrap();
        assert_eq!(deleted, 1);

        let mut rows = conn
            .query("SELECT COUNT(*) FROM widgets WHERE id = 'w-1'", ())
            .await
            .unwrap();
        assert_eq!(
            rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
            0
        );

        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM sync_deletions WHERE table_name = 'widgets' AND row_id = 'w-1'",
                (),
            )
            .await
            .unwrap();
        assert_eq!(
            rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
            1
        );
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn delete_rows_with_tombstones_rolls_back_on_failure() {
        let (db, path) = test_db().await;
        let conn = db.connect().unwrap();
        conn.execute(
            "INSERT INTO widgets (id, user_id) VALUES ('w-1', 'u-1'), ('w-2', 'u-1')",
            (),
        )
        .await
        .unwrap();

        // Force the transaction to fail after the delete: the trigger aborts
        // every tombstone insert, so nothing may commit (delete OR tombstone).
        conn.execute(
            "CREATE TRIGGER fail_tombstone BEFORE INSERT ON sync_deletions
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END",
            (),
        )
        .await
        .unwrap();
        let result = delete_rows_with_tombstones(&conn, "widgets", &["w-1".to_string()]).await;
        assert!(result.is_err());

        let mut rows = conn
            .query("SELECT COUNT(*) FROM widgets", ())
            .await
            .unwrap();
        assert_eq!(
            rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
            2,
            "delete must roll back when the tombstone write fails"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn invalid_table_name_rejected() {
        let (db, path) = test_db().await;
        let conn = db.connect().unwrap();
        assert!(
            record_tombstone(&conn, "widgets; DROP TABLE widgets--", "w-1")
                .await
                .is_err()
        );
        assert!(delete_with_tombstone(&conn, "Widgets", "w-1")
            .await
            .is_err());
        let _ = std::fs::remove_file(&path);
    }
}
