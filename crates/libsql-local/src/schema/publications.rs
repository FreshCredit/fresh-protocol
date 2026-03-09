//! Publications schema definitions
//!
//! Contains publication-related tables for academic/research publications:
//! - publication_records: Canonical publication snapshots from public registries (ORCID, OpenAlex)
//! - publication_claims: User-asserted claims binding records to their identity
//! - publication_evidence: Supporting evidence for verification level upgrades
//! - publication_events: Audit log for all claim state transitions
//! - publication_disputes: User disputes for false matches or corrections
//! - orcid_connections: ORCID identity anchoring for users
//!
//! DATA TAXONOMY: User-Claimed Public Attribution
//! - Underlying records are public (ORCID, OpenAlex, Crossref)
//! - Binding to user is user-asserted, user-controlled, revocable
//! - FreshCredit does not assert underlying facts, only stores user's claim
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! COMPLIANCE: §6 data staging approval only, not credit decisioning

use anyhow::Result;
use libsql::Connection;

use super::try_create_index;
use tracing::info;

/// Initialize publication records table (canonical snapshots from public registries)
pub async fn initialize_publication_records_table(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing publications tables");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS publication_records (
            id TEXT PRIMARY KEY,
            publication_type TEXT NOT NULL CHECK(publication_type IN ('paper', 'dataset', 'chapter', 'preprint', 'thesis', 'book', 'conference')),
            -- Primary identifiers
            doi TEXT UNIQUE,
            openalex_id TEXT UNIQUE,
            orcid_put_code TEXT,
            -- Metadata
            title TEXT NOT NULL,
            publication_date TEXT,
            venue TEXT,
            venue_issn TEXT,
            publisher TEXT,
            -- Authors (JSON array of {name, orcid?, affiliation?})
            authors TEXT NOT NULL,
            -- Classification
            concepts TEXT,
            open_access_status TEXT CHECK(open_access_status IN ('gold', 'green', 'hybrid', 'bronze', 'closed', NULL)),
            is_open_access BOOLEAN DEFAULT FALSE,
            cited_by_count INTEGER DEFAULT 0,
            -- Enrichment tracking
            enriched_at DATETIME,
            -- Snapshot
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
        "CREATE INDEX IF NOT EXISTS idx_publication_records_doi ON publication_records(doi)",
    )
    .await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_publication_records_openalex ON publication_records(openalex_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_publication_records_type ON publication_records(publication_type)").await?;

    Ok(())
}

/// Initialize publication claims table (user-asserted bindings)
pub async fn initialize_publication_claims_table(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing publications tables");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS publication_claims (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            publication_record_id TEXT NOT NULL,
            claim_type TEXT NOT NULL CHECK(claim_type IN (
                'author', 'corresponding_author', 'contributor', 'editor', 'reviewer'
            )),
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
            FOREIGN KEY (publication_record_id) REFERENCES publication_records (id),
            UNIQUE(user_id, publication_record_id, claim_type)
        )",
        (),
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_publication_claims_user_id ON publication_claims(user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_publication_claims_status ON publication_claims(status)",
    )
    .await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_publication_claims_visibility ON publication_claims(visibility)").await?;

    Ok(())
}

/// Initialize ORCID connections table (identity anchoring)
pub async fn initialize_orcid_connections_table(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing publications tables");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS orcid_connections (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL UNIQUE,
            orcid_id TEXT NOT NULL,
            display_name TEXT,
            email TEXT,
            -- OAuth tokens (encrypted)
            access_token_encrypted TEXT,
            refresh_token_encrypted TEXT,
            token_expires_at DATETIME,
            -- Sync state
            last_sync_at DATETIME,
            works_count INTEGER DEFAULT 0,
            -- Status
            status TEXT DEFAULT 'active' CHECK(status IN ('active', 'disconnected', 'expired')),
            connected_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            disconnected_at DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    try_create_index(conn, "CREATE UNIQUE INDEX IF NOT EXISTS idx_orcid_connections_orcid_id ON orcid_connections(orcid_id)").await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_orcid_connections_status ON orcid_connections(status)",
    )
    .await?;

    Ok(())
}

/// Initialize publication evidence table (supporting documents for verification)
pub async fn initialize_publication_evidence_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS publication_evidence (
            id TEXT PRIMARY KEY,
            claim_id TEXT NOT NULL,
            evidence_type TEXT NOT NULL CHECK(evidence_type IN (
                'orcid_confirmation', 'co_author_attestation', 'institutional_verification',
                'doi_metadata_match', 'publication_certificate', 'other'
            )),
            content_hash TEXT,
            content_url TEXT,
            metadata TEXT,
            verified BOOLEAN DEFAULT FALSE,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (claim_id) REFERENCES publication_claims (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_publication_evidence_claim_id ON publication_evidence(claim_id)").await?;

    Ok(())
}

/// Initialize publication events table (audit log)
pub async fn initialize_publication_events_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS publication_events (
            id TEXT PRIMARY KEY,
            claim_id TEXT,
            publication_record_id TEXT,
            event_type TEXT NOT NULL CHECK(event_type IN (
                'staged', 'approved', 'verified', 'revoked', 'disputed', 'refreshed', 'shared'
            )),
            event_data TEXT,
            actor_id TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_publication_events_claim_id ON publication_events(claim_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_publication_events_created_at ON publication_events(created_at)").await?;

    Ok(())
}

/// Initialize publication disputes table
pub async fn initialize_publication_disputes_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS publication_disputes (
            id TEXT PRIMARY KEY,
            claim_id TEXT NOT NULL,
            dispute_type TEXT NOT NULL CHECK(dispute_type IN ('false_match', 'incorrect_data', 'authorship_challenge')),
            reason TEXT NOT NULL,
            status TEXT DEFAULT 'pending' CHECK(status IN ('pending', 'investigating', 'resolved', 'dismissed')),
            resolution_notes TEXT,
            submitted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            resolved_at DATETIME,
            FOREIGN KEY (claim_id) REFERENCES publication_claims (id)
        )",
        (),
    )
    .await?;

    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_publication_disputes_claim_id ON publication_disputes(claim_id)").await?;

    Ok(())
}

/// Initialize publication shares table (provider consent for accessing publications)
pub async fn initialize_publication_shares_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS publication_shares (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            -- Scope of sharing
            share_scope TEXT DEFAULT 'all' CHECK(share_scope IN ('all', 'selected', 'verified_only')),
            selected_claim_ids TEXT,  -- JSON array of claim IDs if scope is 'selected'
            -- Consent details
            status TEXT DEFAULT 'active' CHECK(status IN ('active', 'revoked', 'expired')),
            granted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            expires_at DATETIME,
            revoked_at DATETIME,
            -- Audit
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            UNIQUE(user_id, provider_id)
        )",
        (),
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_publication_shares_user_id ON publication_shares(user_id)",
    )
    .await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_publication_shares_provider_id ON publication_shares(provider_id)").await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_publication_shares_status ON publication_shares(status)",
    )
    .await?;

    Ok(())
}

/// Initialize all publication tables
///
/// Table count: 7 tables
/// - publication_records: Canonical snapshots from public registries (ORCID, OpenAlex)
/// - publication_claims: User-asserted bindings
/// - publication_evidence: Supporting documents
/// - publication_events: Audit log
/// - publication_disputes: User disputes
/// - orcid_connections: ORCID identity anchoring
/// - publication_shares: Provider consent for accessing publications
pub async fn initialize_publication_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing publications tables");
    initialize_publication_records_table(conn).await?;
    initialize_publication_claims_table(conn).await?;
    initialize_orcid_connections_table(conn).await?;
    initialize_publication_evidence_table(conn).await?;
    initialize_publication_events_table(conn).await?;
    initialize_publication_disputes_table(conn).await?;
    initialize_publication_shares_table(conn).await?;
    Ok(())
}
