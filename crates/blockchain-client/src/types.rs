//! Blockchain client types and request/response structures

use serde::{Deserialize, Serialize};

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Request to anchor a report hash on-chain
#[derive(Debug, Serialize)]
pub struct CreateHashRequest {
    /// User identifier
    pub user_id: String,
    /// Type of report being anchored
    pub report_type: String,
    /// Report payload data
    pub report_data: serde_json::Value,
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Response from anchoring a hash
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateHashResponse {
    /// Hash record identifier
    pub id: String,
    /// User identifier
    pub user_id: String,
    /// Type of report anchored
    pub report_type: String,
    /// Anchored hash value
    pub hash: String,
    /// Block number where anchored
    pub block_number: u64,
    /// Timestamp of anchoring
    pub timestamp: String,
}

/// Response from querying a hash
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GetHashResponse {
    /// Hash record identifier
    pub id: String,
    /// User identifier
    pub user_id: String,
    /// Type of report anchored
    pub report_type: String,
    /// Anchored hash value
    pub hash: String,
    /// Block number where anchored
    pub block_number: u64,
    /// Timestamp of anchoring
    pub timestamp: String,
    /// Report payload data
    pub report_data: serde_json::Value,
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Response from verifying a hash
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerifyHashResponse {
    /// Whether the hash was verified successfully
    pub verified: bool,
    /// Hash value that was verified
    pub hash: String,
    /// Block number where anchored (if found)
    pub block_number: Option<u64>,
    /// Timestamp of anchoring (if found)
    pub timestamp: Option<String>,
    /// User identifier (if found)
    pub user_id: Option<String>,
    /// Report type (if found)
    pub report_type: Option<String>,
}

/// Transaction details (Task 1.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainTransaction {
    /// Transaction hash
    pub hash: String,
    /// Amount in smallest unit (plancks for DOT)
    pub amount: u64,
    /// From address
    pub from: String,
    /// To address
    pub to: String,
    /// Block number
    pub block_number: u64,
    /// Extrinsic index in block
    pub extrinsic_index: u32,
}
