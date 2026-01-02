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
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! COMPLIANCE: §1 Money, Custody, and Transactions - NO internal wallets or balances
//!   - crypto_wallets stores EXTERNAL wallet references only (addresses)
//!   - FreshCredit never holds custody of crypto assets
//!   - All crypto flows through external providers (WalletConnect, Circle)

use anyhow::Result;
use libsql::Connection;

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
            status TEXT DEFAULT 'unverified',
            verification_type TEXT,
            is_default BOOLEAN DEFAULT FALSE,
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

    // Index for fast wallet lookups
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_crypto_wallets_user_id
         ON crypto_wallets(user_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_crypto_wallets_address_network
         ON crypto_wallets(wallet_address, network)",
        (),
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

    // Index for payment lookups
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_crypto_payments_user_id
         ON crypto_payments(user_id)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_crypto_payments_status
         ON crypto_payments(status)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_crypto_payments_tx_hash
         ON crypto_payments(tx_hash)",
        (),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_payments_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}

