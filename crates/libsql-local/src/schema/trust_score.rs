//! General Theory of Trust (GTT) schema definitions
//!
//! Edge-side physiological, financial, and linguistic stability streams plus the
//! composite trust index. Mirrors Appendix C.2 of the GTT whitepaper and the
//! A.11 pre-analysis plan (`User_Meta`, labels).
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
const TRUST_SCORE_TABLE_DDL: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS gtt_physio_stream (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            ts_utc DATETIME NOT NULL,
            hr_rest REAL,
            hrv_rmssd REAL,
            sleep_eff REAL,
            temp_skin REAL,
            step_count INTEGER,
            respiration_rate REAL,
            vo2_estimated REAL,
            blood_oxygen REAL,
            psi_bio REAL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    "CREATE TABLE IF NOT EXISTS gtt_fin_stream (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            ts_utc DATETIME NOT NULL,
            inflow REAL,
            outflow REAL,
            balance_mean REAL,
            balance_var REAL,
            merchant_diversity REAL,
            cashflow_30d_inflow REAL,
            cashflow_30d_outflow REAL,
            cashflow_60d_inflow REAL,
            cashflow_60d_outflow REAL,
            cashflow_90d_inflow REAL,
            cashflow_90d_outflow REAL,
            income_regularity REAL,
            spending_entropy REAL,
            psi_fin REAL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    "CREATE TABLE IF NOT EXISTS gtt_ling_stream (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            ts_utc DATETIME NOT NULL,
            msg_len INTEGER,
            emoji_count INTEGER,
            emoji_entropy REAL,
            semantic_cos REAL,
            sentiment_score REAL,
            speech_entropy REAL,
            pitch_variability REAL,
            sentiment_valence REAL,
            sentiment_arousal REAL,
            semantic_distance REAL,
            behavioral_latency_ms REAL,
            behavioral_choice_consistency REAL,
            behavioral_variance REAL,
            device_time_jitter_ms REAL,
            device_battery_percent REAL,
            environment_noise_db REAL,
            psi_ling REAL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    "CREATE TABLE IF NOT EXISTS gtt_composite_index (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            ts_utc DATETIME NOT NULL,
            psi_bio REAL,
            psi_fin REAL,
            psi_ling REAL,
            psi_trust REAL,
            lambda_est REAL,
            delta_h REAL,
            curvature REAL,
            z_entropy_norm REAL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    "CREATE TABLE IF NOT EXISTS gtt_model_metadata (
            id TEXT PRIMARY KEY,
            model_hash TEXT,
            model_version TEXT,
            training_epoch INTEGER,
            loss_val REAL,
            auc_val REAL,
            fairness_ratio REAL,
            chain_tx TEXT,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
    "CREATE TABLE IF NOT EXISTS gtt_user_meta (
            user_id TEXT PRIMARY KEY,
            device_id TEXT,
            hardware_model TEXT,
            region_code TEXT,
            consent_version TEXT,
            hash_ref TEXT,
            ts_utc DATETIME NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    "CREATE TABLE IF NOT EXISTS gtt_user_baseline (
            user_id TEXT PRIMARY KEY,
            hr_rest_mean REAL,
            hr_rest_sd REAL,
            hrv_rmssd_mean REAL,
            hrv_rmssd_sd REAL,
            sleep_eff_mean REAL,
            sleep_eff_sd REAL,
            window_count INTEGER,
            updated_at DATETIME NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    "CREATE TABLE IF NOT EXISTS gtt_labels (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            ts_utc DATETIME NOT NULL,
            label_default_90d INTEGER,
            label_stability_composite REAL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    "CREATE TABLE IF NOT EXISTS gtt_balance_snapshots (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            date TEXT NOT NULL,
            total_balance REAL NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
];

const TRUST_SCORE_INDEX_DDL: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_gtt_physio_stream_user_ts ON gtt_physio_stream(user_id, ts_utc)",
    "CREATE INDEX IF NOT EXISTS idx_gtt_fin_stream_user_ts ON gtt_fin_stream(user_id, ts_utc)",
    "CREATE INDEX IF NOT EXISTS idx_gtt_ling_stream_user_ts ON gtt_ling_stream(user_id, ts_utc)",
    "CREATE INDEX IF NOT EXISTS idx_gtt_composite_index_user_ts ON gtt_composite_index(user_id, ts_utc)",
    "CREATE INDEX IF NOT EXISTS idx_gtt_model_metadata_hash ON gtt_model_metadata(model_hash)",
    "CREATE INDEX IF NOT EXISTS idx_gtt_user_meta_user ON gtt_user_meta(user_id)",
    "CREATE INDEX IF NOT EXISTS idx_gtt_user_baseline_user ON gtt_user_baseline(user_id)",
    "CREATE INDEX IF NOT EXISTS idx_gtt_labels_user_ts ON gtt_labels(user_id, ts_utc)",
    "CREATE INDEX IF NOT EXISTS idx_gtt_balance_snapshots_user_date ON gtt_balance_snapshots(user_id, date)",
];

/// Initialize GTT trust-score tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_trust_score_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing GTT trust-score tables");

    freshcredit_libsql_common::schema::ensure_table_ddls(conn, TRUST_SCORE_TABLE_DDL).await?;

    initialize_trust_score_indexes(conn).await?;

    Ok(())
}

/// Initialize GTT trust-score indexes
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_trust_score_indexes(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing GTT trust-score indexes");

    for sql in TRUST_SCORE_INDEX_DDL {
        conn.execute(sql, ()).await?;
    }

    Ok(())
}
