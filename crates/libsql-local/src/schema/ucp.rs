//! UCP (Universal Commerce Protocol) schema definitions
//!
//! Contains UCP-related tables:
//! - ucp_checkout_sessions: Checkout session lifecycle
//! - ucp_orders: Completed orders from checkout sessions
//! - ucp_identity_links: OAuth 2.0 identity linking
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! COMPLIANCE: AGENT-004 - All queries include user_id filter

use anyhow::Result;
use libsql::Connection;

use super::try_create_index;

/// Initialize all UCP tables
pub async fn initialize_ucp_tables(conn: &Connection) -> Result<()> {
    // UCP Checkout Sessions
    // Stores checkout session lifecycle per UCP v2026-01-11
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ucp_checkout_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL UNIQUE,
            user_id TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            buyer_email TEXT,
            buyer_first_name TEXT,
            buyer_last_name TEXT,
            buyer_identity_token TEXT,
            line_items_json TEXT NOT NULL,
            totals_json TEXT NOT NULL,
            checkout_url TEXT NOT NULL,
            success_url TEXT NOT NULL,
            cancel_url TEXT NOT NULL,
            payment_intent_id TEXT,
            expires_at DATETIME NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            completed_at DATETIME,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // UCP Orders
    // Created from completed checkout sessions
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ucp_orders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            order_id TEXT NOT NULL UNIQUE,
            user_id TEXT NOT NULL,
            checkout_session_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            buyer_email TEXT,
            buyer_first_name TEXT,
            buyer_last_name TEXT,
            line_items_json TEXT NOT NULL,
            totals_json TEXT NOT NULL,
            payment_reference TEXT,
            provider_id INTEGER NOT NULL,
            fulfillment_method TEXT,
            fulfillment_tracking_url TEXT,
            fulfillment_estimated_delivery DATETIME,
            fulfillment_notes TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (checkout_session_id) REFERENCES ucp_checkout_sessions (session_id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // UCP Identity Links
    // OAuth 2.0 identity linking per UCP v2026-01-11
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ucp_identity_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            link_id TEXT NOT NULL UNIQUE,
            client_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            scope TEXT NOT NULL,
            redirect_uri TEXT NOT NULL,
            state TEXT NOT NULL,
            code_verifier TEXT,
            access_token TEXT,
            refresh_token TEXT,
            token_expires_at DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            linked_at DATETIME,
            revoked_at DATETIME,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Create indexes for efficient queries
    // AGENT-004: All queries filter by user_id
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ucp_sessions_user_id ON ucp_checkout_sessions (user_id)",
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ucp_sessions_status ON ucp_checkout_sessions (status)",
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ucp_sessions_buyer ON ucp_checkout_sessions (buyer_email)",
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ucp_orders_user_id ON ucp_orders (user_id)",
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ucp_orders_provider ON ucp_orders (provider_id)",
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ucp_orders_status ON ucp_orders (status)",
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ucp_links_user_id ON ucp_identity_links (user_id)",
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ucp_links_client ON ucp_identity_links (client_id)",
    )
    .await?;

    Ok(())
}

