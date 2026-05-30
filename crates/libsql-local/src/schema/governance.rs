//! Governance system schema definitions
//!
//! Contains governance tables:
//! - `governance_proposals`: Governance proposals for voting
//! - `governance_votes`: Individual votes on proposals
//! - `governance_delegations`: Voting power delegations
//! - `governance_treasury`: Treasury balance and allocations
//! - `governance_treasury_transactions`: Treasury transaction history
//! - `governance_stewards`: Platform stewards with approval authority
//!
//! COMPLIANCE: §7 Terminology and Copy Enforcement - Uses neutral governance terminology
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize governance system tables
/// # Errors
///
/// Returns an error if the operation fails.
#[allow(clippy::too_many_lines)]
pub async fn initialize_governance_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing governance tables");
    // Governance proposals table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS governance_proposals (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            author_id TEXT NOT NULL,
            proposal_type TEXT NOT NULL DEFAULT 'general',
            status TEXT NOT NULL DEFAULT 'draft',
            voting_start_at DATETIME,
            voting_end_at DATETIME,
            quorum_required INTEGER NOT NULL DEFAULT 50,
            vote_for_count INTEGER NOT NULL DEFAULT 0,
            vote_against_count INTEGER NOT NULL DEFAULT 0,
            vote_abstain_count INTEGER NOT NULL DEFAULT 0,
            total_voting_power INTEGER NOT NULL DEFAULT 0,
            execution_data TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            executed_at DATETIME,
            FOREIGN KEY (author_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Governance votes table
    conn.execute(
        // TAG: surface=database owner=platform-team rule=GENERAL-001
        "CREATE TABLE IF NOT EXISTS governance_votes (
            id TEXT PRIMARY KEY,
            proposal_id TEXT NOT NULL,
            voter_id TEXT NOT NULL,
            vote TEXT NOT NULL,
            voting_power INTEGER NOT NULL DEFAULT 1,
            signature TEXT,
            reason TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (proposal_id) REFERENCES governance_proposals (id) ON DELETE CASCADE,
            FOREIGN KEY (voter_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            UNIQUE (proposal_id, voter_id)
        )",
        (),
    )
    .await?;

    // Governance delegations table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS governance_delegations (
            id TEXT PRIMARY KEY,
            delegator_id TEXT NOT NULL,
            delegate_id TEXT NOT NULL,
            voting_power INTEGER NOT NULL DEFAULT 1,
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            revoked_at DATETIME,
            FOREIGN KEY (delegator_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (delegate_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            UNIQUE (delegator_id, delegate_id)
        )",
        (),
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // Governance treasury table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS governance_treasury (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            balance INTEGER NOT NULL DEFAULT 0,
            currency TEXT NOT NULL DEFAULT 'USDC',
            allocation_development INTEGER NOT NULL DEFAULT 40,
            allocation_operations INTEGER NOT NULL DEFAULT 30,
            allocation_grants INTEGER NOT NULL DEFAULT 20,
            allocation_reserve INTEGER NOT NULL DEFAULT 10,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    // Governance treasury transactions table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS governance_treasury_transactions (
            id TEXT PRIMARY KEY,
            treasury_id TEXT NOT NULL,
            transaction_type TEXT NOT NULL,
            amount INTEGER NOT NULL,
            description TEXT NOT NULL,
            proposal_id TEXT,
            executed_by TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (treasury_id) REFERENCES governance_treasury (id) ON DELETE CASCADE,
            FOREIGN KEY (proposal_id) REFERENCES governance_proposals (id) ON DELETE SET NULL,
            FOREIGN KEY (executed_by) REFERENCES user_profile (id) ON DELETE SET NULL
        )",
        (),
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // Governance stewards table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS governance_stewards (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL UNIQUE,
            steward_role TEXT NOT NULL DEFAULT 'steward',
            status TEXT NOT NULL DEFAULT 'active',
            term_starts_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            term_ends_at DATETIME,
            appointed_by TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (appointed_by) REFERENCES user_profile (id) ON DELETE SET NULL
        )",
        (),
    )
    .await?;

    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize governance indexes
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_governance_indexes(conn: &Connection) -> Result<()> {
    use super::try_create_index;

    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_governance_proposals_status ON governance_proposals (status)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_governance_proposals_author ON governance_proposals (author_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_governance_votes_proposal ON governance_votes (proposal_id)").await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_governance_votes_voter ON governance_votes (voter_id)",
    )
    .await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_governance_delegations_delegator ON governance_delegations (delegator_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_governance_delegations_delegate ON governance_delegations (delegate_id)").await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_governance_stewards_user ON governance_stewards (user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_governance_stewards_status ON governance_stewards (status)",
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_governance_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}
