# NOMT Client

**Version:** 0.1.0  
**Status:** Active  
**Purpose:** HTTP client for NOMT server API

---

## Overview

This crate provides a client for interacting with the NOMT HTTP API, used by the blockchain client and other services that need to store and verify hashes.

## Quick Start

```rust
use nomt_client::NomtClient;

// Create client
let client = NomtClient::new("http://localhost:8080")?;

// Store data and get proof
let response = client.store(
    "data to hash",
    Some(HashMetadata::new("report", "report-123")),
).await?;

// Verify a hash
let verify = client.verify(response.hash, Some(response.root)).await?;
assert!(verify.verified);
```

## Features

- Store data and receive Merkle proofs
- Verify hashes against roots
- Get storage statistics
- Prepare data for blockchain anchoring

## Anchoring Workflow

```rust
// Store and get proof for blockchain anchoring
let proof = client.prepare_anchor(
    "data to anchor",
    Some(HashMetadata::new("contract", "contract-456")),
).await?;

// proof can now be submitted to blockchain
```

## Related Crates

- `nomt-core` - Core types (NomtProof)
- `nomt-server` - The server this client connects to
- `blockchain-client` - Uses this crate for anchoring

## License

MIT OR Apache-2.0
