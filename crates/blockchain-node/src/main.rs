// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001 test-coverage=unit
// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001 test-coverage=e2e
//! `FreshCredit` Blockchain Explorer
//!
//! Web-based blockchain explorer for the Substrate-based `FreshCredit` node.
//! Displays blocks, anchored hashes, and chain statistics.
//!
//! ## Unified Verification Architecture
//!
//! The `/api/hash/:hash` endpoint implements a cascading verification strategy:
//! 1. NOMT sidecar (fast, Merkle proof available)
//! 2. Substrate on-chain (authoritative, slower)
//!
//! This ensures hashes stored via NOMT are immediately verifiable while
//! maintaining backward compatibility with directly-anchored hashes.

// Module-level dead code allowances removed as part of P0 cleanup
// Individual issues should be fixed rather than suppressed globally
// See: RESEARCH_SYNTHESIS.md for cleanup details

#![allow(clippy::wildcard_imports)]
#![allow(clippy::significant_drop_tightening)]

mod cli;
mod config;
mod helpers;
mod node;
mod rpc;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = cli::Cli::parse();
    node::run(cli).await
}
