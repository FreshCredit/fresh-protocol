// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001 test-coverage=unit
//! CLI argument parsing.

use clap::Parser;

#[derive(Parser)]
#[command(name = "freshcredit-explorer")]
#[command(about = "FreshCredit Blockchain Explorer")]
pub struct Cli {
    /// Port to listen on (also reads from PORT env var for Cloud Run)
    #[arg(long, env = "PORT", default_value = "9933")]
    pub port: u16,

    /// Substrate node WebSocket URL (also reads from `SUBSTRATE_URL` env var)
    #[arg(long, env = "SUBSTRATE_URL")]
    pub substrate_url: String,

    /// NOMT sidecar URL (also reads from `NOMT_SIDECAR_URL` env var)
    #[arg(long, env = "NOMT_SIDECAR_URL")]
    pub nomt_sidecar_url: Option<String>,
}
