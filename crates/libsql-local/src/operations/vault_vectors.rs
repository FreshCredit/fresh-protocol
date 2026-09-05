//! Vault category vector persistence and search.
//!
//! Each user receives one row per vault category (`financial`, `professional`,
//! `personal`, `health`, `identity`, ...) containing a deterministic feature
//! vector derived from the user's vault contents. Vectors are stored as native
//! libSQL `F32_BLOB`s and queried through the `vector_top_k` `DiskANN` index.
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use tracing::info;

use crate::schema::vault_vectors::VAULT_VECTOR_DIM;
use crate::LocalClient;

/// One stored category vector.
#[derive(Debug, Clone)]
pub struct VaultCategoryVector {
    /// Row id (libSQL `ROWID`).
    pub id: i64,
    /// User that owns the vector.
    pub user_id: String,
    /// Scope: `consumer` or `provider`.
    pub scope: String,
    /// Category pocket.
    pub category: String,
    /// Source ids that contributed to the vector, serialized as JSON.
    pub source_ids: String,
    /// Vector components.
    pub vector: Vec<f32>,
    /// Last update timestamp.
    pub updated_at: String,
}

/// Similarity search result.
#[derive(Debug, Clone)]
pub struct VaultVectorSearchResult {
    /// Matched vector row.
    pub vector: VaultCategoryVector,
    /// Cosine distance (0 = identical, 2 = opposite).
    pub distance: f64,
}

// TAG: surface=database owner=platform-team rule=DB-001
impl LocalClient {
    /// Store or replace a category vector for a user.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn store_vault_category_vector(
        &self,
        user_id: &str,
        scope: &str,
        category: &str,
        source_ids: &[String],
        vector: &[f32],
    ) -> Result<()> {
        let source_ids_json = serde_json::to_string(source_ids)?;
        let vector_literal = format_vector32_literal(vector);
        let now = chrono::Utc::now().to_rfc3339();
        let sql = format!(
            "INSERT INTO vault_category_vectors
             (user_id, scope, category, source_ids, vector, updated_at, created_at)
             VALUES (?1, ?2, ?3, ?4, vector32({vector_literal}), ?5, ?5)
             ON CONFLICT(user_id, scope, category) DO UPDATE SET
                 source_ids = excluded.source_ids,
                 vector = excluded.vector,
                 updated_at = excluded.updated_at"
        );
        self.connection()
            .execute(
                &sql,
                libsql::params![user_id, scope, category, source_ids_json, now],
            )
            .await?;
        Ok(())
    }

    /// Get every stored category vector for a user/scope.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_vault_category_vectors(
        &self,
        user_id: &str,
        scope: &str,
    ) -> Result<Vec<VaultCategoryVector>> {
        let mut rows = self
            .connection()
            .query(
                "SELECT id, user_id, scope, category, source_ids,
                        vector_extract(vector), updated_at
                 FROM vault_category_vectors
                 WHERE user_id = ?1 AND scope = ?2
                 ORDER BY category",
                libsql::params![user_id, scope],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(parse_vector_row(&row)?);
        }
        Ok(out)
    }

    /// Get a single stored category vector.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_vault_category_vector(
        &self,
        user_id: &str,
        scope: &str,
        category: &str,
    ) -> Result<Option<VaultCategoryVector>> {
        let mut rows = self
            .connection()
            .query(
                "SELECT id, user_id, scope, category, source_ids,
                        vector_extract(vector), updated_at
                 FROM vault_category_vectors
                 WHERE user_id = ?1 AND scope = ?2 AND category = ?3",
                libsql::params![user_id, scope, category],
            )
            .await?;
        let row = rows.next().await?;
        row.as_ref().map(parse_vector_row).transpose()
    }

    /// Delete all category vectors for a user/scope.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn delete_vault_category_vectors(&self, user_id: &str, scope: &str) -> Result<u64> {
        let count = self
            .connection()
            .execute(
                "DELETE FROM vault_category_vectors WHERE user_id = ?1 AND scope = ?2",
                libsql::params![user_id, scope],
            )
            .await?;
        Ok(count)
    }

    /// Approximate nearest-neighbor search over category vectors using the
    /// libSQL `vector_top_k` table-valued function.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn search_vault_category_vectors(
        &self,
        query_vector: &[f32],
        k: usize,
    ) -> Result<Vec<VaultVectorSearchResult>> {
        let vector_literal = format_vector32_literal(query_vector);
        let sql = format!(
            "SELECT id, user_id, scope, category, source_ids,
                    vector_extract(vector), updated_at,
                    vector_distance_cos(vector, vector32({vector_literal})) AS distance
             FROM vault_category_vectors
             WHERE rowid IN (
                 SELECT id FROM vector_top_k('vault_category_vectors_idx', vector32({vector_literal}), {k})
             )
             ORDER BY distance ASC
             LIMIT {k}"
        );
        info!("[vault_vectors] ANN search k={}", k);
        let mut rows = self.connection().query(&sql, ()).await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let distance: f64 = row.get(7)?;
            out.push(VaultVectorSearchResult {
                vector: parse_vector_row(&row)?,
                distance,
            });
        }
        Ok(out)
    }
}

/// Format a slice of f32 values as a libSQL `vector32('[...]')` literal.
fn format_vector32_literal(vector: &[f32]) -> String {
    let values = vector
        .iter()
        .map(|v| format!("{v:.6}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("'[{values}]'")
}

/// Parse a vector row. Column layout must match the SELECT lists above.
fn parse_vector_row(row: &libsql::Row) -> Result<VaultCategoryVector> {
    let vector_text: String = row.get(5)?;
    let vector = parse_vector_text(&vector_text)?;
    Ok(VaultCategoryVector {
        id: row.get(0)?,
        user_id: row.get(1)?,
        scope: row.get(2)?,
        category: row.get(3)?,
        source_ids: row.get(4)?,
        vector,
        updated_at: row.get(6)?,
    })
}

/// Parse the JSON array text returned by `vector_extract`.
fn parse_vector_text(text: &str) -> Result<Vec<f32>> {
    let trimmed = text.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(trimmed);
    let mut vector = Vec::with_capacity(VAULT_VECTOR_DIM);
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        vector.push(part.parse::<f32>()?);
    }
    Ok(vector)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_vector() -> Vec<f32> {
        let dim = u16::try_from(VAULT_VECTOR_DIM).expect("test vector dim fits u16");
        (0..dim).map(|i| f32::from(i) * 0.05).collect()
    }

    #[tokio::test]
    async fn vector_round_trip() {
        let client = LocalClient::new_in_memory().await.unwrap();
        client.initialize_schema().await.unwrap();

        client
            .store_vault_category_vector(
                "user-1",
                "consumer",
                "financial",
                &["plaid".to_string()],
                &test_vector(),
            )
            .await
            .unwrap();

        let rows = client
            .get_vault_category_vectors("user-1", "consumer")
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].category, "financial");
        assert_eq!(rows[0].vector.len(), VAULT_VECTOR_DIM);
        assert!((rows[0].vector[3] - 0.15).abs() < 0.0001);
    }

    #[tokio::test]
    async fn vector_search_finds_nearest() {
        let client = LocalClient::new_in_memory().await.unwrap();
        client.initialize_schema().await.unwrap();

        let mut financial = vec![0.0f32; VAULT_VECTOR_DIM];
        financial[0] = 1.0;
        let mut professional = vec![0.0f32; VAULT_VECTOR_DIM];
        professional[1] = 1.0;

        client
            .store_vault_category_vector(
                "user-1",
                "consumer",
                "financial",
                &["plaid".to_string()],
                &financial,
            )
            .await
            .unwrap();
        client
            .store_vault_category_vector(
                "user-1",
                "consumer",
                "professional",
                &["linkedin".to_string()],
                &professional,
            )
            .await
            .unwrap();

        let mut query = vec![0.0f32; VAULT_VECTOR_DIM];
        query[0] = 1.0;
        let results = client
            .search_vault_category_vectors(&query, 5)
            .await
            .unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].vector.category, "financial");
    }

    #[test]
    fn parse_vector_text_handles_output() {
        let parsed = parse_vector_text("[0.1, 0.2, 0.3]").unwrap();
        assert_eq!(parsed, vec![0.1, 0.2, 0.3]);
    }
}
