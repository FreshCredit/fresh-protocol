use super::*;
pub async fn initialize_provider_tables(conn: &Connection) -> Result<()> {
    // COMPLIANCE: §2 - Neutral matching only, no recommendations
    conn.execute(
        "CREATE TABLE IF NOT EXISTS provider_offers (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            product_type TEXT NOT NULL,
            loan_amount_min_cents INTEGER,
            loan_amount_max_cents INTEGER,
            apr_min_percent REAL,
            apr_max_percent REAL,
            term_options_months TEXT,
            fees_json TEXT,
            rewards_json TEXT,
            included_states TEXT,
            excluded_states TEXT,
            included_zip_codes TEXT,
            nationwide BOOLEAN DEFAULT TRUE,
            customer_segment TEXT DEFAULT 'all',
            customer_profiles TEXT,
            blockscore_model_id TEXT,
            ab_test_variant TEXT,
            ab_test_allocation INTEGER,
            is_active BOOLEAN DEFAULT TRUE,
            version INTEGER DEFAULT 1,
            blockchain_hash TEXT,
            block_number INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (provider_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS disputes (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            dispute_type TEXT NOT NULL,
            disputed_item_type TEXT,
            disputed_item_id TEXT,
            dispute_reason TEXT NOT NULL,
            dispute_description TEXT,
            supporting_documents TEXT,
            dispute_status TEXT DEFAULT 'pending',
            resolution TEXT,
            resolved_at DATETIME,
            raw_dispute_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}

/// Initialize verification tables
