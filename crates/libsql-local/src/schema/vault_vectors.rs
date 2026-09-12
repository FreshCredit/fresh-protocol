//! Vault category vector schema
//!
//! Stores deterministic, category-specific feature vectors computed from the
//! user's vault contents. Vectors are fixed-length `F32_BLOB`s and are indexed
//! with libSQL's native `DiskANN` vector index so category similarity search is
//! fast without an external embedding service.
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;

use super::try_create_index;

/// Dimension of every vault category vector.
pub const VAULT_VECTOR_DIM: usize = 16;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize the vault category vector table and its vector index.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_vault_vector_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS vault_category_vectors (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            scope TEXT NOT NULL,
            category TEXT NOT NULL,
            source_ids TEXT NOT NULL DEFAULT '[]',
            vector F32_BLOB({VAULT_VECTOR_DIM}),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(user_id, scope, category)
        )"
        ),
        (),
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_vault_category_vectors_user_scope_category \
         ON vault_category_vectors(user_id, scope, category)",
    )
    .await?;

    // DiskANN vector index for native similarity search. Creation is
    // idempotent; if the running libSQL build lacks vector support this will
    // fail and propagate so callers know vector search is unavailable.
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS vault_category_vectors_idx \
         ON vault_category_vectors (libsql_vector_idx(vector, 'metric=cosine'))",
    )
    .await?;

    Ok(())
}
