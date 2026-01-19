//! UCP (Universal Commerce Protocol) schema definitions
//!
//! Contains UCP-related tables:
//! - ucp_checkout_sessions: Checkout session lifecycle
//! - ucp_orders: Completed orders from checkout sessions
//! - ucp_identity_links: OAuth 2.0 identity linking
//! - ucp_merchants: Verified merchant directory for UCP discovery
//! - user_offer_engagements: User journey tracking from offer view to tradeline
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

    // User Offer Engagements
    // Tracks the complete user journey: offer view → checkout → payment → tradeline
    // This enables analytics and provides a unified view of user engagement
    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_offer_engagements (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            offer_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'viewed',

            -- Journey timestamps
            viewed_at DATETIME,
            selected_at DATETIME,
            checkout_started_at DATETIME,
            payment_completed_at DATETIME,
            tradeline_created_at DATETIME,

            -- Reference IDs (progressively populated)
            checkout_session_id TEXT,
            payment_id TEXT,
            ucp_order_id TEXT,
            tradeline_account_id TEXT,

            -- Offer terms snapshot (captured at engagement time)
            loan_amount REAL,
            apr REAL,
            term_months INTEGER,
            monthly_payment REAL,

            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,

            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (checkout_session_id) REFERENCES ucp_checkout_sessions (session_id),
            FOREIGN KEY (ucp_order_id) REFERENCES ucp_orders (order_id)
        )",
        (),
    )
    .await?;

    // Indexes for user_offer_engagements
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_engagements_user_id ON user_offer_engagements (user_id)",
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_engagements_offer_id ON user_offer_engagements (offer_id)",
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_engagements_status ON user_offer_engagements (status)",
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_engagements_user_offer ON user_offer_engagements (user_id, offer_id)",
    )
    .await?;

    // UCP Merchants
    // Verified merchant directory for UCP discovery protocol
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ucp_merchants (
            id TEXT PRIMARY KEY,
            domain TEXT NOT NULL UNIQUE,
            profile_url TEXT NOT NULL,
            display_name TEXT NOT NULL,
            is_verified INTEGER NOT NULL DEFAULT 0,
            capabilities TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        (),
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ucp_merchants_domain ON ucp_merchants(domain)",
    )
    .await?;

    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_ucp_merchants_verified ON ucp_merchants(is_verified)",
    )
    .await?;

    Ok(())
}

