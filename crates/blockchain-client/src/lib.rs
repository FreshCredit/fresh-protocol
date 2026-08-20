// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001 test-coverage=unit
// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001 test-coverage=integration
// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001 test-coverage=api
#![allow(clippy::multiple_crate_versions)]
//! `FreshCredit` Substrate Blockchain Client
//!
//! Client for interacting with the `FreshCredit` Substrate node via subxt.
//! Uses Substrate's JSON-RPC for chain queries and extrinsic submission.
//!
//! ## Architecture
//! - Uses subxt for type-safe Substrate RPC calls
//! - Service account keypair for auto-signing extrinsics
//! - SHA-256 based `user_id` to `AccountId32` mapping

// Generate runtime interface from downloaded metadata
// HARDCODED_CONFIG: Metadata file version - must match current pallet-freshcredit runtime
// To update: Run `subxt metadata --url http://localhost:9944 -f bytes > freshcredit_metadata_v{N}.scale`
// Then update the path below and bump the version number
#[subxt::subxt(runtime_metadata_path = "freshcredit_metadata_v1.scale")]
pub mod freshcredit_runtime {}

pub mod bulletin;
pub mod client;
pub mod helpers;
pub mod nomt;
pub mod types;

pub use bulletin::{compute_cid, BulletinClient, BulletinConfig, BulletinStoreResult};
pub use client::BlockchainClient;
pub use nomt::{NomtAnchorResult, NomtClient, NomtProof};
pub use report_type::ReportType;
pub use types::{
    BlockchainTransaction, CreateHashRequest, CreateHashResponse, GetHashResponse,
    VerifyHashResponse,
};

mod report_type;
#[cfg(test)]
mod tests;
