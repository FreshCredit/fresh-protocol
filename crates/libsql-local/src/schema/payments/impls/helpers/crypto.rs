use anyhow::Result;
use libsql::Connection;

use crate::schema::try_create_index;

/// Initialize crypto tables: crypto_wallets and crypto_payments.
pub async fn create_crypto_tables(conn: &Connection) -> Result<()> {
    // Crypto wallet connections (WalletConnect / Circle Wallets)
    // COMPLIANCE: §1 - This stores REFERENCES to external wallets only
    // FreshCredit never holds custody - wallets are controlled by users or Circle
    conn.execute(
        "CREATE TABLE IF NOT EXISTS crypto_wallets (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            -- Wallet address (e.g., 0x123...)
            wallet_address TEXT NOT NULL,
            -- Provider: 'walletconnect', 'circle', 'external'
            wallet_provider TEXT NOT NULL,
            -- Network: 'arc', 'base', 'ethereum', 'polygon'
            network TEXT NOT NULL DEFAULT 'arc',
            -- For Circle wallets: Circle wallet ID
            circle_wallet_id TEXT,
            -- For WalletConnect: session topic for reconnection
            walletconnect_session_topic TEXT,
            -- Wallet name/label provided by user
            label TEXT,
            -- Whether this is the default wallet for payments
            is_default BOOLEAN DEFAULT FALSE,
            -- Whether this wallet is active (user can disable without deleting)
            is_active BOOLEAN DEFAULT TRUE,
            -- Active/disconnected status
            status TEXT DEFAULT 'active',
            -- Last used timestamp for cleanup
            last_used_at DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // Index for fast wallet lookups (using defensive helper for cloud schema compatibility)
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_crypto_wallets_user_id ON crypto_wallets(user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_crypto_wallets_address_network ON crypto_wallets(wallet_address, network)",
    )
    .await?;

    // Crypto payment transactions with Arc settlement
    // COMPLIANCE: §1 - Payments via external wallets, receipts on Arc
    conn.execute(
        "CREATE TABLE IF NOT EXISTS crypto_payments (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            -- Reference to the wallet used for payment
            wallet_id TEXT NOT NULL,
            -- Payment amount in smallest unit (e.g., 6 decimals for USDC)
            amount TEXT NOT NULL,
            -- Token: 'USDC', 'EURC'
            token TEXT NOT NULL DEFAULT 'USDC',
            -- Network where payment was made
            network TEXT NOT NULL DEFAULT 'arc',
            -- Payment status: 'pending', 'confirming', 'confirmed', 'failed'
            status TEXT DEFAULT 'pending',
            -- On-chain transaction hash
            tx_hash TEXT UNIQUE,
            -- Block number when confirmed
            block_number INTEGER,
            -- Gas used for the transaction
            gas_used TEXT,
            -- Arc receipt transaction hash (L1 settlement proof)
            arc_receipt_tx_hash TEXT,
            -- Substrate anchor hash (L0 verification)
            substrate_hash TEXT,
            -- Recipient wallet address
            recipient_address TEXT NOT NULL,
            -- Description/memo
            description TEXT,
            -- Error message if failed
            failure_reason TEXT,
            -- Timestamps
            initiated_at DATETIME,
            confirmed_at DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (wallet_id) REFERENCES crypto_wallets (id) ON DELETE SET NULL
        )",
        (),
    )
    .await?;

    // Index for payment lookups (using defensive helper for cloud schema compatibility)
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_crypto_payments_user_id ON crypto_payments(user_id)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_crypto_payments_status ON crypto_payments(status)",
    )
    .await?;
    try_create_index(
        conn,
        "CREATE INDEX IF NOT EXISTS idx_crypto_payments_tx_hash ON crypto_payments(tx_hash)",
    )
    .await?;

    Ok(())
}
