---
title: "blockchain-client"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "blockchain"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# blockchain-client

Substrate blockchain integration for FreshCredit via subxt.

## Purpose

Provides a type-safe client for interacting with the FreshCredit Substrate blockchain node. Uses subxt for type-safe RPC calls to anchor report hashes on-chain, verify data integrity, and query blockchain state. Includes NOMT sidecar integration for off-chain Merkle proof storage.

## Directory Structure

```
crates/blockchain-client/
├── README.md                     # This file
├── Cargo.toml                    # Crate manifest
├── freshcredit_metadata_v1.scale # Substrate runtime metadata
└── src/
    ├── lib.rs                    # Main module with client implementation
    ├── nomt.rs                   # NOMT sidecar client
    └── types.rs                  # Type definitions
```

## Key Types/Traits

| Name | Type | Description |
|------|------|-------------|
| `BlockchainClient` | struct | Main Substrate client |
| `CreateHashRequest` | struct | Request to anchor a hash |
| `CreateHashResponse` | struct | Response from hash anchoring |
| `GetHashResponse` | struct | Retrieved hash data |
| `VerifyHashResponse` | struct | Hash verification result |
| `ReportType` | enum | Types of reports: PlaidFinancial, IdentityVerification, etc. |
| `BlockchainTransaction` | struct | Transaction details |
| `NomtClient` | struct | NOMT sidecar client |
| `NomtProof` | struct | Merkle proof for verification |
| `NomtAnchorResult` | struct | NOMT storage result |

## Usage

### Basic Example

```rust
use blockchain_client::{BlockchainClient, CreateHashRequest, ReportType};

// Create client from environment
let client = BlockchainClient::from_env().await?;

// Check health
let is_healthy = client.health_check().await?;

// Anchor a report hash
let request = CreateHashRequest {
    user_id: "user_123".to_string(),
    report_type: "plaid_financial".to_string(),
    report_data: serde_json::json!({ /* report data */ }),
};
let response = client.create_hash(request).await?;
```

### Hash Verification

```rust
// Verify a hash exists on-chain
let verification = client.verify_hash(&hash).await?;
if verification.verified {
    println!("Hash confirmed at block {}", 
        verification.block_number.unwrap());
}
```

### NOMT Sidecar Integration

```rust
use blockchain_client::NomtClient;

// Connect to NOMT sidecar
let nomt = NomtClient::new("http://localhost:8081");

// Store data and get proof
let result = nomt.store(user_id, report_type, data).await?;
println!("NOMT root: {}", result.nomt_root);

// Verify data
let is_valid = nomt.verify(&hash).await?;
```

### Blockchain Queries

```rust
// Get blockchain statistics
let stats = client.get_stats().await?;

// Get recent blocks
let blocks = client.get_recent_blocks(10).await?;

// Get specific block
let block = client.get_block(block_number).await?;
```

## Dependencies

### External
- `subxt` / `subxt-signer` - Substrate client with sr25519 signing
- `sp-crypto-hashing` - Substrate hashing for transaction IDs
- `anyhow` - Error handling
- `serde` / `serde_json` - Serialization
- `tracing` - Logging
- `tokio` - Async runtime
- `sha2` - SHA-256 hashing
- `chrono` - Date/time handling
- `hex` - Hex encoding
- `reqwest` - HTTP client for NOMT

### Internal
- `freshcredit-config` - Environment configuration

## Architecture

```
FreshCredit API
    ↓
BlockchainClient (subxt)
    ↓
Substrate Node (consensus, finality)
    ↓
NOMT Sidecar (off-chain storage)
```

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `BLOCKCHAIN_RPC_URL` | No | WebSocket RPC endpoint (default: ws://localhost:9944) |
| `BLOCKCHAIN_SIGNER_MNEMONIC` | Recommended | Signer mnemonic for production |
| `BLOCKCHAIN_SIGNER_SURI` | No | Signer SURI for development |
| `NOMT_SIDECAR_URL` | No | NOMT sidecar endpoint |

## Signer Priority

1. `BLOCKCHAIN_SIGNER_MNEMONIC` (production)
2. `BLOCKCHAIN_SIGNER_SURI` (development)
3. `//Alice` (default for local development)

## Testing

```bash
# Run all blockchain client tests
cargo test -p blockchain-client

# Note: Some tests require local Substrate node running
```

## Related Documentation

📁 [NOMT Sidecar](../nomt-sidecar/) - Off-chain Merkle trie storage

📁 [NOMT WASM](../nomt-wasm/) - Browser proof verification

📁 [Arc Poller](../arc-poller/) - Arc Network integration

📁 [Architecture Overview](../../../docs/application/architecture/)
