// TAG: surface=security owner=security-team rule=SEC-001
//! Database schema initialization for `LibSQL` session storage

use super::core::LibSqlSessionStorage;
use crate::storage::SessionError;
use std::collections::HashSet;
use tracing::{debug, info};

impl LibSqlSessionStorage {
    /// Initialize the sessions table
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn init(&self) -> Result<(), SessionError> {
        // Create the sessions table with device info and refresh token support
        // TAG: surface=security owner=security-team rule=SEC-001
        let create_table = r"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                last_activity INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                data TEXT NOT NULL,
                user_agent TEXT,
                ip_address TEXT,
                device_type TEXT,
                os TEXT,
                browser TEXT,
                country TEXT,
                access_token TEXT,
                refresh_token_encrypted TEXT,
                refresh_token_expires_at INTEGER,
                token_rotation_count INTEGER DEFAULT 0,
                last_refresh_at INTEGER
            )
        ";
        self.conn.execute(create_table, ()).await?;

        // P0-FIX: Migrate legacy sessions tables that were created before the
        // token columns were added. `CREATE TABLE IF NOT EXISTS` will not add
        // missing columns, so we inspect the current schema and add any columns
        // that are absent. This keeps local dev databases forward-compatible.
        self.migrate_sessions_columns().await?;

        // Create indexes separately (SQLite doesn't support inline INDEX)
        let _ = self
            .conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id)",
                (),
            )
            .await;
        let _ = self
            .conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at)",
                (),
            )
            .await;
        let _ = self
            .conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_sessions_access_token ON sessions(access_token)",
                (),
            )
            .await;
        let _ = self
            .conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_sessions_refresh_expires ON sessions(refresh_token_expires_at) WHERE refresh_token_encrypted IS NOT NULL",
                (),
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            )
            .await;

        // Create session artifacts table
        let create_artifacts_table = r"
            CREATE TABLE IF NOT EXISTS session_artifacts (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                artifact_type TEXT NOT NULL,
                data TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )
        ";
        self.conn.execute(create_artifacts_table, ()).await?;

        // Create indexes for artifacts
        let _ = self
            .conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_artifacts_session_id ON session_artifacts(session_id)",
                (),
            )
            .await;
        let _ = self
            .conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_artifacts_user_id ON session_artifacts(user_id)",
                (),
            )
                // TAG: surface=security owner=platform-team rule=MID-001
            .await;
        let _ = self
            .conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_artifacts_created_at ON session_artifacts(created_at)",
                (),
            )
            .await;

        info!("Sessions and artifacts tables initialized");
        Ok(())
    }

    /// Add any sessions columns that are missing from older schema versions.
    /// This is a no-op when all columns already exist.
    async fn migrate_sessions_columns(&self) -> Result<(), SessionError> {
        let mut rows = self.conn.query("PRAGMA table_info(sessions)", ()).await?;
        let mut existing = HashSet::new();
        while let Some(row) = rows.next().await? {
            let name: String = row.get(1)?;
            existing.insert(name);
        }

        let migrations: &[(&str, &str)] = &[
            ("access_token", "TEXT"),
            ("refresh_token_encrypted", "TEXT"),
            ("refresh_token_expires_at", "INTEGER"),
            ("token_rotation_count", "INTEGER DEFAULT 0"),
            ("last_refresh_at", "INTEGER"),
        ];

        for (column, col_type) in migrations {
            if existing.contains(*column) {
                debug!("Sessions column {} already exists", column);
                continue;
            }
            let sql = format!("ALTER TABLE sessions ADD COLUMN {column} {col_type}");
            self.conn.execute(&sql, ()).await?;
            info!(
                "Migrated sessions table: added column {} {}",
                column, col_type
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    async fn in_memory_storage() -> LibSqlSessionStorage {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .unwrap();
        let conn = db.connect().unwrap();
        LibSqlSessionStorage::new(Arc::new(conn))
    }

    #[tokio::test]
    async fn test_init_creates_sessions_table() {
        let storage = in_memory_storage().await;
        storage.init().await.unwrap();
        // TAG: surface=security owner=security-team rule=SEC-001

        let mut rows = storage
            .conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='sessions'",
                (),
            )
            .await
            .unwrap();
        assert!(rows.next().await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_init_creates_artifacts_table() {
        let storage = in_memory_storage().await;
        storage.init().await.unwrap();

        let mut rows = storage
            .conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='session_artifacts'",
                (),
            )
            .await
            .unwrap();
        assert!(rows.next().await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_init_creates_indexes() {
        let storage = in_memory_storage().await;
        // TAG: surface=security owner=platform-team rule=MID-001
        storage.init().await.unwrap();

        let mut rows = storage
            .conn
            .query(
                "SELECT COUNT(*) as cnt FROM sqlite_master WHERE type='index' AND name LIKE 'idx_sessions_%'",
                (),
            )
            .await
            .unwrap();
        let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(count, 4);

        let mut rows = storage
            .conn
            .query(
                "SELECT COUNT(*) as cnt FROM sqlite_master WHERE type='index' AND name LIKE 'idx_artifacts_%'",
                (),
            )
            .await
            .unwrap();
        let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_init_is_idempotent() {
        let storage = in_memory_storage().await;
        storage.init().await.unwrap();
        storage.init().await.unwrap();
        // Should not panic or error
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
