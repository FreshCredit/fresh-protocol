//! Teams schema definitions
//!
//! Contains provider team management tables:
//! - `provider_teams`: Team definitions for each provider
//! - `team_members`: Team membership with roles
//! - `team_invites`: Pending invitations to join teams
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! COMPLIANCE: §27.4 Provider Admin Rules - First user is admin

use anyhow::Result;
use libsql::Connection;
use tracing::info;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize teams tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_teams_tables(conn: &Connection) -> Result<()> {
    info!("[ARCH-007] Initializing teams tables");
    // Provider teams - each provider can have multiple teams
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS provider_teams (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            is_default INTEGER DEFAULT 0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (provider_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
    )
    .await?;

    // Team members - links users to teams with roles
    // Roles: admin, manager, viewer
    // Status: active, suspended, removed
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS team_members (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'viewer',
            invited_by TEXT,
            joined_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            status TEXT NOT NULL DEFAULT 'active',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (team_id) REFERENCES provider_teams (id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (invited_by) REFERENCES user_profile (id) ON DELETE SET NULL,
            UNIQUE(team_id, user_id)
        )",
    )
    .await?;

    // Team invites - pending invitations
    // Status: pending, accepted, declined, expired, cancelled
    freshcredit_libsql_common::schema::ensure_table_ddl(
        conn,
        "CREATE TABLE IF NOT EXISTS team_invites (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            email TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'viewer',
            token TEXT NOT NULL UNIQUE,
            invited_by TEXT NOT NULL,
            expires_at DATETIME NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            accepted_by TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (team_id) REFERENCES provider_teams (id) ON DELETE CASCADE,
            FOREIGN KEY (invited_by) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (accepted_by) REFERENCES user_profile (id) ON DELETE SET NULL
        )",
    )
    .await?;

    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize teams indexes
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_teams_indexes(conn: &Connection) -> Result<()> {
    use super::try_create_index;

    // Provider teams indexes
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_provider_teams_provider_id ON provider_teams(provider_id)",
    )
    .await?;

    // Team members indexes
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_team_members_team_id ON team_members(team_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_team_members_user_id ON team_members(user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_team_members_status ON team_members(status)",
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // Team invites indexes
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_team_invites_team_id ON team_invites(team_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_team_invites_email ON team_invites(email)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_team_invites_token ON team_invites(token)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_team_invites_status ON team_invites(status)",
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_teams_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}
