use anyhow::Result;
use libsql::Connection;

use crate::schema::try_create_index;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize Arc receipts table with indexes.
pub async fn create_arc_tables(conn: &Connection) -> Result<()> {
    // Arc receipts - L1 settlement receipts on Circle Arc
    // COMPLIANCE: §1 - Arc is receipt layer, NOT payment processing
    // All receipts are gated by Substrate L0 verification (substrate_hash required)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS arc_receipts (
            id TEXT PRIMARY KEY,
            -- Receipt type: CONSENT_GRANTED, REPORT_ACCESSED, ESCROW_CREATED, SETTLEMENT_COMPLETED, REFUND_ISSUED
            receipt_type TEXT NOT NULL,
            -- Arc transaction hash (the on-chain receipt)
            arc_tx_hash TEXT UNIQUE,
            -- Arc block number where receipt was included
            arc_block_number INTEGER,
            -- Substrate anchor hash (L0 verification - REQUIRED)
            substrate_hash TEXT NOT NULL,
            -- Substrate block number
            substrate_block_number INTEGER NOT NULL,
            -- User who initiated the action
            user_id TEXT,
            -- Provider involved (if applicable)
            provider_id TEXT,
            -- Related deal/transaction ID
            deal_id TEXT,
            -- Amount in cents (for payment-related receipts)
            amount_cents INTEGER,
            -- Gas used for the Arc transaction
            gas_used TEXT,
            -- Receipt status: pending, queued, written, failed
            status TEXT NOT NULL DEFAULT 'pending',
            -- Error message if failed
            failure_reason TEXT,
            -- Retry count for failed receipts
            retry_count INTEGER DEFAULT 0,
            -- Next retry time (for exponential backoff)
            next_retry_at DATETIME,
            // TAG: surface=database owner=data-team rule=DB-001
            -- Optional metadata (JSON)
            metadata TEXT,
            -- Timestamps
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            written_at DATETIME,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE SET NULL
        )",
        (),
    )
    .await?;

    // Index for Arc receipt lookups (using defensive helper for cloud schema compatibility)
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_arc_receipts_user_id ON arc_receipts(user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_arc_receipts_substrate_hash ON arc_receipts(substrate_hash)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_arc_receipts_status ON arc_receipts(status)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_arc_receipts_receipt_type ON arc_receipts(receipt_type)",
    )
    .await?;

    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize bridge transfers table with indexes.
pub async fn create_bridge_tables(conn: &Connection) -> Result<()> {
    // Bridge transfers - Circle Bridge Kit cross-chain USDC transfers (Phase 2A)
    // COMPLIANCE: §1 - Circle manages custody during bridging, not FreshCredit
    conn.execute(
        "CREATE TABLE IF NOT EXISTS bridge_transfers (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            -- Source wallet reference
            source_wallet_id TEXT NOT NULL,
            -- Destination wallet reference
            destination_wallet_id TEXT NOT NULL,
            -- Source chain: 'arc', 'ethereum', 'base', 'polygon', etc.
            source_chain TEXT NOT NULL,
            -- Destination chain
            destination_chain TEXT NOT NULL,
            -- Amount in smallest unit (6 decimals for USDC)
            amount TEXT NOT NULL,
            -- Token being bridged (always USDC for now)
            token TEXT NOT NULL DEFAULT 'USDC',
            -- Transfer status: 'pending', 'initiated', 'confirming', 'completed', 'failed', 'cancelled'
            status TEXT NOT NULL DEFAULT 'pending',
            -- Source chain transaction hash
            source_tx_hash TEXT,
            -- Destination chain transaction hash
            destination_tx_hash TEXT,
            -- Circle transfer ID (from Bridge Kit API)
            circle_transfer_id TEXT UNIQUE,
            -- Estimated completion time
            estimated_completion DATETIME,
            -- Actual completion time
            completed_at DATETIME,
            -- Fee charged for the bridge
            // TAG: surface=database owner=data-team rule=DB-001
            fee TEXT,
            -- Error message if failed
            error_message TEXT,
            -- Timestamps
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (source_wallet_id) REFERENCES crypto_wallets (id) ON DELETE SET NULL,
            FOREIGN KEY (destination_wallet_id) REFERENCES crypto_wallets (id) ON DELETE SET NULL
        )",
        (),
    )
    .await?;

    // Index for bridge transfer lookups
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_bridge_transfers_user_id ON bridge_transfers(user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_bridge_transfers_status ON bridge_transfers(status)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_bridge_transfers_circle_id ON bridge_transfers(circle_transfer_id)",
    )
    .await?;

    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize gateway tables: `gateway_sessions` and `gateway_transactions`.
pub async fn create_gateway_tables(conn: &Connection) -> Result<()> {
    // Gateway sessions - Circle Gateway fiat on/off ramp sessions (Phase 2B)
    // COMPLIANCE: §1 - Circle Gateway handles fiat custody, not FreshCredit
    conn.execute(
        "CREATE TABLE IF NOT EXISTS gateway_sessions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            -- Wallet to receive/send USDC
            wallet_id TEXT NOT NULL,
            -- Session type: 'on_ramp' (buy USDC) or 'off_ramp' (sell USDC)
            session_type TEXT NOT NULL,
            -- Session status: 'created', 'pending', 'processing', 'completed', 'failed', 'expired'
            status TEXT NOT NULL DEFAULT 'created',
            -- Circle session ID
            circle_session_id TEXT UNIQUE,
            -- Widget URL for user to complete the transaction
            widget_url TEXT,
            -- Fiat currency: 'USD', 'EUR', 'GBP', etc.
            fiat_currency TEXT NOT NULL DEFAULT 'USD',
            -- Fiat amount (if known)
            fiat_amount TEXT,
            -- USDC amount (if known)
            usdc_amount TEXT,
            -- Session expiration time
            expires_at DATETIME,
            -- Timestamps
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (wallet_id) REFERENCES crypto_wallets (id) ON DELETE SET NULL
        )",
        (),
    )
    .await?;

    // Index for gateway session lookups
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_gateway_sessions_user_id ON gateway_sessions(user_id)",
        // TAG: surface=database owner=platform-team rule=DB-001
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_gateway_sessions_status ON gateway_sessions(status)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_gateway_sessions_circle_id ON gateway_sessions(circle_session_id)",
    )
    .await?;

    // Gateway transactions - Circle Gateway fiat transactions (Phase 2B)
    // COMPLIANCE: §1 - Circle Gateway handles fiat custody, not FreshCredit
    conn.execute(
        "CREATE TABLE IF NOT EXISTS gateway_transactions (
            id TEXT PRIMARY KEY,
            -- Reference to the gateway session
            session_id TEXT NOT NULL,
            -- User who initiated the transaction
            user_id TEXT NOT NULL,
            -- Transaction type: 'on_ramp' or 'off_ramp'
            transaction_type TEXT NOT NULL,
            -- Fiat currency
            fiat_currency TEXT NOT NULL,
            -- Fiat amount
            fiat_amount TEXT NOT NULL,
            -- USDC amount
            usdc_amount TEXT NOT NULL,
            -- Exchange rate at time of transaction
            exchange_rate TEXT,
            -- Fee charged
            fee TEXT,
            -- On-chain transaction hash (for USDC transfer)
            // TAG: surface=database owner=data-team rule=DB-001
            tx_hash TEXT,
            -- Bank reference (for fiat transfer)
            bank_reference TEXT,
            -- Transaction status: 'pending', 'processing', 'completed', 'failed'
            status TEXT NOT NULL DEFAULT 'pending',
            -- Error message if failed
            error_message TEXT,
            -- Timestamps
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            completed_at DATETIME,
            FOREIGN KEY (session_id) REFERENCES gateway_sessions (id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Index for gateway transaction lookups
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_gateway_transactions_session_id ON gateway_transactions(session_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_gateway_transactions_user_id ON gateway_transactions(user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_gateway_transactions_status ON gateway_transactions(status)",
    )
    .await?;

    Ok(())
}
