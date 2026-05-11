//! Intellectual Property schema definitions
//!
//! Contains IP-related tables for patent, trademark, and copyright claims:
//! - `ip_records`: Canonical IP record snapshots from public registries
//! - `ip_claims`: User-asserted claims binding records to their identity
//! - `ip_evidence`: Supporting evidence for verification level upgrades
//! - `ip_events`: Audit log for all claim state transitions
//! - `ip_disputes`: User disputes for false matches or corrections
//!
//! DATA TAXONOMY: User-Claimed Public Attribution
//! - Underlying records are public (USPTO, Copyright Office)
//! - Binding to user is user-asserted, user-controlled, revocable
//! - `FreshCredit` does not assert underlying facts, only stores user's claim
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! COMPLIANCE: §6 data staging approval only, not credit decisioning

use anyhow::Result;
use libsql::Connection;

use super::try_create_index;
use tracing::info;

/// Initialize IP records table (canonical snapshots from public registries)
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_ip_records_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ip_records (
            id TEXT PRIMARY KEY,
            ip_type TEXT NOT NULL CHECK(ip_type IN ('patent', 'trademark', 'copyright')),
            external_id TEXT NOT NULL,
            stable_identifier TEXT UNIQUE,
            title TEXT,
            filing_date TEXT,
            grant_date TEXT,
            status TEXT,
            canonical_snapshot TEXT NOT NULL,
            source TEXT NOT NULL,
            source_url TEXT,
            last_fetched_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ip_records_type ON ip_records(ip_type)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ip_records_external_id ON ip_records(external_id)",
    )
    .await?;

    Ok(())
}

/// Initialize IP claims table (user-asserted bindings)
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_ip_claims_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ip_claims (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            ip_record_id TEXT NOT NULL,
            claim_type TEXT NOT NULL,
            verification_level INTEGER DEFAULT 0 CHECK(verification_level BETWEEN 0 AND 4),
            verification_details TEXT,
            visibility TEXT DEFAULT 'private' CHECK(visibility IN ('private', 'conditional', 'shared')),
            status TEXT DEFAULT 'pending' CHECK(status IN ('pending', 'approved', 'revoked', 'disputed')),
            staged_at DATETIME,
            approved_at DATETIME,
            revoked_at DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (ip_record_id) REFERENCES ip_records (id)
        )",
        (),
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ip_claims_user_id ON ip_claims(user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ip_claims_status ON ip_claims(status)",
    )
    .await?;

    Ok(())
}

/// Initialize IP evidence table (supporting documents for verification)
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_ip_evidence_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ip_evidence (
            id TEXT PRIMARY KEY,
            claim_id TEXT NOT NULL,
            evidence_type TEXT NOT NULL,
            content_hash TEXT,
            content_url TEXT,
            metadata TEXT,
            verified BOOLEAN DEFAULT FALSE,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (claim_id) REFERENCES ip_claims (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ip_evidence_claim_id ON ip_evidence(claim_id)",
    )
    .await?;

    Ok(())
}

/// Initialize IP events table (audit log)
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_ip_events_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ip_events (
            id TEXT PRIMARY KEY,
            claim_id TEXT,
            ip_record_id TEXT,
            event_type TEXT NOT NULL,
            event_data TEXT,
            actor_id TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ip_events_claim_id ON ip_events(claim_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ip_events_created_at ON ip_events(created_at)",
    )
    .await?;

    Ok(())
}

/// Initialize IP disputes table
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_ip_disputes_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ip_disputes (
            id TEXT PRIMARY KEY,
            claim_id TEXT NOT NULL,
            dispute_type TEXT NOT NULL,
            reason TEXT NOT NULL,
            status TEXT DEFAULT 'pending' CHECK(status IN ('pending', 'investigating', 'resolved', 'dismissed')),
            resolution_notes TEXT,
            submitted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            resolved_at DATETIME,
            FOREIGN KEY (claim_id) REFERENCES ip_claims (id)
        )",
        (),
    )
    .await?;

    Ok(())
}

/// Initialize all IP tables
///
/// Table count: 5 tables
/// - `ip_records`: Canonical snapshots from public registries
/// - `ip_claims`: User-asserted bindings
/// - `ip_evidence`: Supporting documents
/// - `ip_events`: Audit log
/// - `ip_disputes`: User disputes
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_ip_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing ip tables");
    initialize_ip_records_table(conn).await?;
    initialize_ip_claims_table(conn).await?;
    initialize_ip_evidence_table(conn).await?;
    initialize_ip_events_table(conn).await?;
    initialize_ip_disputes_table(conn).await?;
    Ok(())
}
