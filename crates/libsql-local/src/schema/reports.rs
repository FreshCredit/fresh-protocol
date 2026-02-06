//! Reports and scoring schema definitions
//!
//! Contains report and offer tables:
//! - reports: Generated reports (BlockID)
//! - scores: Provider-defined scoring models (BlockScore)
//! - offers: Matched offers (BlockIQ)
//! - provider_offers: Provider product catalog
//! - disputes: Consumer disputes
//! - verification_requests: Data verification requests
//! - phone_verification_codes: SMS verification
//! - blockchain_proofs: NOMT/Substrate proof storage for offline verification
//!
//! COMPLIANCE: §2 - Neutral matching only, no recommendations
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;
use tracing::info;

/// Initialize reports and scoring tables
pub async fn initialize_reports_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing reports tables");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS reports (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            report_type TEXT NOT NULL,
            report_name TEXT,
            report_status TEXT DEFAULT 'pending',
            generation_started_at DATETIME,
            generation_completed_at DATETIME,
            expiration_date DATETIME,
            blockchain_hash TEXT,
            blockchain_tx_id TEXT,
            report_data TEXT,
            raw_report_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // NOTE: FreshCredit does NOT generate scores - providers define their own models
    conn.execute(
        "CREATE TABLE IF NOT EXISTS scores (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            score_model_id TEXT NOT NULL,
            score_model_name TEXT,
            score_model_version TEXT,
            provider_calculated_score INTEGER,
            provider_score_factors TEXT,
            data_elements_used TEXT,
            data_element_weights TEXT,
            calculated_at DATETIME,
            expires_at DATETIME,
            raw_score_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // NOTE: FreshCredit matches offers, does not recommend them
    conn.execute(
        "CREATE TABLE IF NOT EXISTS offers (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            offer_type TEXT NOT NULL,
            offer_name TEXT,
            offer_description TEXT,
            match_score REAL,
            match_criteria TEXT,
            user_preferences_matched TEXT,
            provider_requirements_matched TEXT,
            offer_terms TEXT,
            offer_amount_min REAL,
            offer_amount_max REAL,
            offer_apr_min REAL,
            offer_apr_max REAL,
            offer_duration_months INTEGER,
            offer_status TEXT DEFAULT 'active',
            user_viewed_at DATETIME,
            user_clicked_at DATETIME,
            user_applied_at DATETIME,
            expires_at DATETIME,
            raw_offer_data TEXT NOT NULL,
            blockchain_hash TEXT,
            block_number INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}

/// Initialize provider offers and dispute tables
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
pub async fn initialize_verification_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS verification_requests (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            request_type TEXT NOT NULL,
            requested_data_elements TEXT,
            consent_given BOOLEAN DEFAULT FALSE,
            consent_timestamp DATETIME,
            consent_expires_at DATETIME,
            request_status TEXT DEFAULT 'pending',
            verification_result TEXT,
            verified_at DATETIME,
            raw_request_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS phone_verification_codes (
            id TEXT PRIMARY KEY,
            user_id TEXT,
            phone_number TEXT NOT NULL,
            code TEXT NOT NULL,
            purpose TEXT NOT NULL DEFAULT 'phone_verification',
            attempts INTEGER DEFAULT 0,
            max_attempts INTEGER DEFAULT 3,
            expires_at DATETIME NOT NULL,
            verified_at DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS verified_credentials (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            credential_type TEXT NOT NULL,
            credential_issuer TEXT NOT NULL,
            credential_subject TEXT,
            credential_id TEXT UNIQUE,
            did_uri TEXT,
            issuance_date DATETIME,
            expiration_date DATETIME,
            credential_status TEXT DEFAULT 'active',
            revocation_id TEXT,
            credential_data TEXT,
            raw_credential_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}

/// Initialize offer analytics tables
pub async fn initialize_offer_analytics_tables(conn: &Connection) -> Result<()> {
    // Daily offer analytics metrics
    conn.execute(
        "CREATE TABLE IF NOT EXISTS offer_analytics (
            id TEXT PRIMARY KEY,
            offer_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            date TEXT NOT NULL,
            impressions INTEGER DEFAULT 0,
            views INTEGER DEFAULT 0,
            clicks INTEGER DEFAULT 0,
            applications INTEGER DEFAULT 0,
            conversions INTEGER DEFAULT 0,
            revenue_cents INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (offer_id) REFERENCES provider_offers (id) ON DELETE CASCADE,
            UNIQUE (offer_id, date)
        )",
        (),
    )
    .await?;

    // A/B test results for offer variants
    conn.execute(
        "CREATE TABLE IF NOT EXISTS offer_ab_test_results (
            id TEXT PRIMARY KEY,
            offer_id TEXT NOT NULL,
            variant TEXT NOT NULL,
            impressions INTEGER DEFAULT 0,
            conversions INTEGER DEFAULT 0,
            revenue_cents INTEGER DEFAULT 0,
            start_date TEXT NOT NULL,
            end_date TEXT,
            is_active INTEGER DEFAULT 1,
            statistical_significance REAL,
            winner INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (offer_id) REFERENCES provider_offers (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Offer lifecycle events for conversion tracking
    conn.execute(
        "CREATE TABLE IF NOT EXISTS offer_events (
            id TEXT PRIMARY KEY,
            offer_id TEXT NOT NULL,
            user_id TEXT,
            event_type TEXT NOT NULL,
            event_data TEXT,
            session_id TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (offer_id) REFERENCES provider_offers (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}

/// Initialize offer analytics indexes
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
/// COMPLIANCE: §1 - blockchain_proofs is browser-side for offline verification
async fn initialize_blockchain_proofs_table(conn: &Connection) -> Result<()> {
    conn.execute(
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
        (),
    )
    .await?;

    // Indexes for blockchain_proofs
    super::try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_blockchain_proofs_hash ON blockchain_proofs (hash)",
    )
    .await?;

    super::try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_blockchain_proofs_user_id ON blockchain_proofs (user_id)",
    )
    .await?;

    Ok(())
}

/// Initialize all reports and provider tables
pub async fn initialize_all_reports_tables(conn: &Connection) -> Result<()> {
    initialize_reports_tables(conn).await?;
    initialize_provider_tables(conn).await?;
    seed_demo_provider_offers(conn).await?;
    initialize_verification_tables(conn).await?;
    initialize_offer_analytics_tables(conn).await?;
    initialize_offer_analytics_indexes(conn).await?;
    initialize_blockchain_proofs_table(conn).await?;
    Ok(())
}

/// Seed demo provider offers if table is empty
/// COMPLIANCE: §4 - Template rules - these are example offers for demonstration
/// NOTE: These are demo offers. Providers must define their own terms in production.
async fn seed_demo_provider_offers(conn: &Connection) -> Result<()> {
    // Check if offers already exist
    let mut rows = conn.query("SELECT COUNT(*) as count FROM provider_offers", ()).await?;
    if let Some(row) = rows.next().await? {
        let count: i64 = row.get(0)?;
        if count > 0 {
            return Ok(()); // Already seeded
        }
    }

    // First, create demo provider user profiles (required for foreign key constraint)
    // These are system demo providers, not real users
    let demo_providers = [
        ("demo_provider_001", "FlexiLoan Financial", "flexiloan@demo.freshcredit.com"),
        ("demo_provider_002", "Rewards Plus Bank", "rewardsplus@demo.freshcredit.com"),
        ("demo_provider_003", "AutoDrive Finance", "autodrive@demo.freshcredit.com"),
        ("demo_provider_004", "HomeFirst Lending", "homefirst@demo.freshcredit.com"),
        ("demo_provider_005", "EduFund Services", "edufund@demo.freshcredit.com"),
        ("demo_provider_006", "BizFlex Capital", "bizflex@demo.freshcredit.com"),
    ];

    for (id, name, email) in demo_providers {
        // Use INSERT OR IGNORE to avoid duplicates
        let _ = conn.execute(
            "INSERT OR IGNORE INTO user_profile (id, platform_user_id, azure_id, email, display_name, role, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, 'provider', datetime('now'), datetime('now'))",
            libsql::params![id, id, id, email, name],
        ).await;
    }

    // Seed demo offers
    conn.execute(
        "INSERT INTO provider_offers (id, provider_id, name, description, product_type,
            loan_amount_min_cents, loan_amount_max_cents, apr_min_percent, apr_max_percent,
            term_options_months, nationwide, is_active, created_at, updated_at)
         VALUES ('demo_offer_personal_001', 'demo_provider_001', 'FlexiLoan Personal',
            'Flexible personal loan with competitive rates', 'PersonalLoan',
            500000, 5000000, 7.99, 24.99, '[12, 24, 36, 48, 60]', 1, 1, datetime('now'), datetime('now'))",
        (),
    ).await?;

    conn.execute(
        "INSERT INTO provider_offers (id, provider_id, name, description, product_type,
            loan_amount_min_cents, loan_amount_max_cents, apr_min_percent, apr_max_percent,
            term_options_months, nationwide, is_active, created_at, updated_at)
         VALUES ('demo_offer_credit_001', 'demo_provider_002', 'Rewards Plus Card',
            'Earn 2% cashback on all purchases with no annual fee', 'CreditCard',
            50000, 2500000, 15.99, 26.99, '[]', 1, 1, datetime('now'), datetime('now'))",
        (),
    ).await?;

    conn.execute(
        "INSERT INTO provider_offers (id, provider_id, name, description, product_type,
            loan_amount_min_cents, loan_amount_max_cents, apr_min_percent, apr_max_percent,
            term_options_months, nationwide, is_active, created_at, updated_at)
         VALUES ('demo_offer_auto_001', 'demo_provider_003', 'AutoDrive Financing',
            'New and used vehicle financing with flexible terms', 'AutoLoan',
            1000000, 7500000, 4.99, 18.99, '[36, 48, 60, 72, 84]', 1, 1, datetime('now'), datetime('now'))",
        (),
    ).await?;

    conn.execute(
        "INSERT INTO provider_offers (id, provider_id, name, description, product_type,
            loan_amount_min_cents, loan_amount_max_cents, apr_min_percent, apr_max_percent,
            term_options_months, included_states, nationwide, is_active, created_at, updated_at)
         VALUES ('demo_offer_mortgage_001', 'demo_provider_004', 'HomeFirst Mortgage',
            'Conventional and FHA mortgage options for home buyers', 'Mortgage',
            10000000, 100000000, 5.25, 7.50, '[180, 240, 360]',
            '[\"CA\", \"TX\", \"NY\", \"FL\", \"WA\"]', 0, 1, datetime('now'), datetime('now'))",
        (),
    ).await?;

    conn.execute(
        "INSERT INTO provider_offers (id, provider_id, name, description, product_type,
            loan_amount_min_cents, loan_amount_max_cents, apr_min_percent, apr_max_percent,
            term_options_months, nationwide, is_active, created_at, updated_at)
         VALUES ('demo_offer_student_001', 'demo_provider_005', 'EduFund Student Loan',
            'Private student loans for undergraduate and graduate students', 'StudentLoan',
            100000, 15000000, 4.49, 14.99, '[60, 84, 120, 180]', 1, 1, datetime('now'), datetime('now'))",
        (),
    ).await?;

    conn.execute(
        "INSERT INTO provider_offers (id, provider_id, name, description, product_type,
            loan_amount_min_cents, loan_amount_max_cents, apr_min_percent, apr_max_percent,
            term_options_months, customer_segment, nationwide, is_active, created_at, updated_at)
         VALUES ('demo_offer_business_001', 'demo_provider_006', 'BizFlex Credit Line',
            'Revolving business credit line for small business owners', 'LineOfCredit',
            1000000, 25000000, 8.99, 22.99, '[12, 24]', 'business', 1, 1, datetime('now'), datetime('now'))",
        (),
    ).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_reports_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}
