//! Reports and scoring schema definitions
//!
//! Contains report and offer tables:
//! - reports: Generated reports (`BlockID`)
//! - scores: Provider-defined scoring models (`BlockScore`)
//! - offers: Matched offers (`BlockIQ`)
//! - `provider_offers`: Provider product catalog
//! - disputes: Consumer disputes
//! - `verification_requests`: Data verification requests
//! - `phone_verification_codes`: SMS verification
//! - `blockchain_proofs`: NOMT/Substrate proof storage for offline verification
//!
//! COMPLIANCE: §2 - Neutral matching only, no recommendations
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;
use tracing::info;

pub mod offer_analytics;
pub mod offer_indexes;
pub mod provider;
pub mod verification;

use offer_indexes::initialize_blockchain_proofs_table;
use provider::seed_demo_provider_offers;

/// Initialize reports and scoring tables
/// # Errors
///
/// Returns an error if the operation fails.
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

/// Initialize all reports and provider tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_all_reports_tables(conn: &Connection) -> Result<()> {
    initialize_reports_tables(conn).await?;
    provider::initialize_provider_tables(conn).await?;
    seed_demo_provider_offers(conn).await?;
    verification::initialize_verification_tables(conn).await?;
    offer_analytics::initialize_offer_analytics_tables(conn).await?;
    offer_indexes::initialize_offer_analytics_indexes(conn).await?;
    initialize_blockchain_proofs_table(conn).await?;
    Ok(())
}
