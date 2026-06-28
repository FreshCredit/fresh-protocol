// TAG: surface=database owner=platform-team rule=DB-001 test-coverage=unit
//! Payment schema definitions
//!
//! Contains payment-related tables:
//! - customers: Stripe/Dwolla customers
//! - `funding_sources`: Payment funding sources
//! - payments: Payment transactions
//! - `stripe_plaid_payments`: Stripe+Plaid ACH payments
//! - `virtual_accounts`: Virtual account numbers
//! - `crypto_wallets`: User crypto wallet connections (WalletConnect/Circle)
//! - `crypto_payments`: Crypto payment transactions with Arc settlement
//! - `arc_receipts`: Circle Arc L1 settlement receipts (gated by Substrate L0)
//! - `bridge_transfers`: Circle Bridge Kit cross-chain USDC transfers (Phase 2A)
//! - `gateway_sessions`: Circle Gateway fiat on/off ramp sessions (Phase 2B)
//! - `gateway_transactions`: Circle Gateway fiat transactions (Phase 2B)
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture
//! COMPLIANCE: §1 Money, Custody, and Transactions - NO internal wallets or balances
//!   - `crypto_wallets` stores EXTERNAL wallet references only (addresses)
//!   - `FreshCredit` never holds custody of crypto assets
//!   - All crypto flows through external providers (`WalletConnect`, Circle)

mod impls;
#[cfg(test)]
mod tests;
mod types;

pub use impls::initialize_payment_tables;
