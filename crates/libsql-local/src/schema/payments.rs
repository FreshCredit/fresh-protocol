//! Payment schema definitions
//!
//! Contains payment-related tables:
//! - customers: Stripe/Dwolla customers
//! - funding_sources: Payment funding sources
//! - payments: Payment transactions
//! - stripe_plaid_payments: Stripe+Plaid ACH payments
//! - virtual_accounts: Virtual account numbers
//! - crypto_wallets: User crypto wallet connections (WalletConnect/Circle)
//! - crypto_payments: Crypto payment transactions with Arc settlement
//! - arc_receipts: Circle Arc L1 settlement receipts (gated by Substrate L0)
//! - bridge_transfers: Circle Bridge Kit cross-chain USDC transfers (Phase 2A)
//! - gateway_sessions: Circle Gateway fiat on/off ramp sessions (Phase 2B)
//! - gateway_transactions: Circle Gateway fiat transactions (Phase 2B)
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! COMPLIANCE: §1 Money, Custody, and Transactions - NO internal wallets or balances
//!   - crypto_wallets stores EXTERNAL wallet references only (addresses)
//!   - FreshCredit never holds custody of crypto assets
//!   - All crypto flows through external providers (WalletConnect, Circle)

use anyhow::Result;
use libsql::Connection;

use super::try_create_index;

/// Initialize all payment tables
pub async fn initialize_payment_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS customers (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            stripe_customer_id TEXT UNIQUE,
            dwolla_customer_id TEXT UNIQUE,
            customer_type TEXT DEFAULT 'consumer',
            email TEXT,
            phone TEXT,
            first_name TEXT,
            last_name TEXT,
            business_name TEXT,
            business_type TEXT,
            status TEXT DEFAULT 'active',
            verification_status TEXT,
            raw_customer_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS funding_sources (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            customer_id TEXT NOT NULL,
            account_id TEXT,
            funding_source_id TEXT UNIQUE,
            funding_source_type TEXT NOT NULL,
            bank_name TEXT,
            bank_account_type TEXT,
            name TEXT,
            last_four TEXT,
            status TEXT DEFAULT 'unverified',
            verification_type TEXT,
            is_default BOOLEAN DEFAULT FALSE,
            is_active BOOLEAN DEFAULT TRUE,
            raw_funding_source_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE,
            FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE SET NULL
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS payments (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            customer_id TEXT NOT NULL,
            funding_source_id TEXT,
            payment_id TEXT UNIQUE,
            payment_type TEXT NOT NULL,
            amount REAL NOT NULL,
            currency TEXT DEFAULT 'USD',
            status TEXT DEFAULT 'pending',
            description TEXT,
            metadata TEXT,
            failure_reason TEXT,
            initiated_at DATETIME,
            completed_at DATETIME,
            raw_payment_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE,
            FOREIGN KEY (funding_source_id) REFERENCES funding_sources (id) ON DELETE SET NULL
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS stripe_plaid_payments (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            stripe_payment_intent_id TEXT UNIQUE,
            plaid_account_id TEXT NOT NULL,
            amount REAL NOT NULL,
            currency TEXT DEFAULT 'USD',
            status TEXT DEFAULT 'pending',
            stripe_customer_id TEXT,
            stripe_payment_method_id TEXT,
            plaid_access_token TEXT,
            description TEXT,
            metadata TEXT,
            failure_reason TEXT,
            initiated_at DATETIME,
            completed_at DATETIME,
            raw_payment_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS virtual_accounts (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            customer_id TEXT NOT NULL,
            virtual_account_id TEXT UNIQUE,
            account_number TEXT,
            routing_number TEXT,
            account_type TEXT DEFAULT 'checking',
            status TEXT DEFAULT 'active',
            balance REAL DEFAULT 0.0,
            currency TEXT DEFAULT 'USD',
            raw_virtual_account_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE,
            FOREIGN KEY (customer_id) REFERENCES customers (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

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
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_crypto_wallets_user_id ON crypto_wallets(user_id)").await?;
    try_create_index(conn, "CREATE UNIQUE INDEX IF NOT EXISTS idx_crypto_wallets_address_network ON crypto_wallets(wallet_address, network)").await?;

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
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_crypto_payments_user_id ON crypto_payments(user_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_crypto_payments_status ON crypto_payments(status)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_crypto_payments_tx_hash ON crypto_payments(tx_hash)").await?;

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
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_arc_receipts_user_id ON arc_receipts(user_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_arc_receipts_substrate_hash ON arc_receipts(substrate_hash)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_arc_receipts_status ON arc_receipts(status)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_arc_receipts_receipt_type ON arc_receipts(receipt_type)").await?;

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
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_bridge_transfers_user_id ON bridge_transfers(user_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_bridge_transfers_status ON bridge_transfers(status)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_bridge_transfers_circle_id ON bridge_transfers(circle_transfer_id)").await?;

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
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_gateway_sessions_user_id ON gateway_sessions(user_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_gateway_sessions_status ON gateway_sessions(status)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_gateway_sessions_circle_id ON gateway_sessions(circle_session_id)").await?;

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
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_gateway_transactions_session_id ON gateway_transactions(session_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_gateway_transactions_user_id ON gateway_transactions(user_id)").await?;
    try_create_index(conn, "CREATE INDEX IF NOT EXISTS idx_gateway_transactions_status ON gateway_transactions(status)").await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_payments_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}

