use crate::schema::try_create_index;
use anyhow::Result;
use libsql::Connection;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize offer analytics indexes
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_offer_analytics_indexes(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_offer_analytics_offer_id ON offer_analytics(offer_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_offer_analytics_date ON offer_analytics(date DESC)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_offer_analytics_provider_id ON offer_analytics(provider_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_offer_ab_test_results_offer_id ON offer_ab_test_results(offer_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_offer_events_offer_id ON offer_events(offer_id)",
        (),
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_offer_events_user_id ON offer_events(user_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_offer_events_created_at ON offer_events(created_at DESC)",
        (),
    )
    .await?;

    Ok(())
}

/// Initialize blockchain proofs table
/// Stores NOMT/Substrate proofs for offline verification via smoldot
/// COMPLIANCE: §1 - `blockchain_proofs` is browser-side for offline verification
pub(crate) async fn initialize_blockchain_proofs_table(conn: &Connection) -> Result<()> {
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS blockchain_proofs (
            id TEXT PRIMARY KEY,
            hash TEXT NOT NULL UNIQUE,
            user_id TEXT,
            proof_type TEXT NOT NULL DEFAULT 'nomt',
            nomt_root TEXT,
            substrate_block_number INTEGER,
            substrate_block_hash TEXT,
            leaf_hash TEXT,
            siblings TEXT,
            path TEXT,
            verified_at DATETIME,
            verification_method TEXT,
            is_trustless INTEGER DEFAULT 0,
            latency_ms INTEGER,
            raw_proof_data TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;

    // Indexes for blockchain_proofs
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_blockchain_proofs_hash ON blockchain_proofs (hash)",
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_blockchain_proofs_user_id ON blockchain_proofs (user_id)",
    )
    .await?;

    Ok(())
}
