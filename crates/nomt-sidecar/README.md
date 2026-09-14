---
title: "nomt-sidecar"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "blockchain"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# nomt-sidecar

NOMT (Nearly-Optimal Merkle Trie) sidecar service for FreshCredit blockchain.

## Purpose

Provides high-performance Merkle trie storage for report data, running alongside the Substrate blockchain node. Stores full report data off-chain while only minimal anchor hashes are stored on-chain. Supports both native NOMT storage (local NVMe) and legacy JSON storage (NFS-compatible) with automatic fallback. Includes periodic root anchoring to Substrate and peer synchronization for StatefulSet deployments.

## Directory Structure

```
crates/nomt-sidecar/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    ├── main.rs         # HTTP/gRPC server entry point
    ├── types.rs        # Type definitions
    ├── storage.rs      # Legacy JSON storage
    ├── native_storage.rs # Native NOMT storage
    ├── anchoring.rs    # Substrate root anchoring
    └── peer_sync.rs    # StatefulSet peer synchronization
```

## Key Types/Traits

| Name | Type | Description |
|------|------|-------------|
| `StorageBackend` | enum | Native or legacy storage backend |
| `NomtStorage` | struct | Legacy JSON-based storage |
| `NativeNomtStorage` | struct | Native NOMT (Beatree + Bitbox) |
| `AnchoringService` | struct | Periodic root anchoring to Substrate |
| `PeerSyncService` | struct | Peer discovery and sync |
| `StoreRequest` | struct | Data storage request |
| `StoreResponse` | struct | Storage response with proof |
| `VerifyResponse` | struct | Hash verification response |
| `NomtProof` | struct | Merkle proof for verification |

## Usage

### Running the Service

```bash
# Native NOMT mode (local disk only)
NOMT_NATIVE=true cargo run -p nomt-sidecar

# Legacy mode (NFS-compatible)
cargo run -p nomt-sidecar
```

### HTTP API

#### Store Data
```bash
POST /store
{
  "user_id": "user_123",
  "report_type": "plaid_financial",
  "data": "base64-encoded-data"
}
```

#### Verify Hash
```bash
GET /verify/{hash}
```

#### Get Proof
```bash
GET /proof/{hash}
```

#### Health Check
```bash
GET /health
GET /ready
```

#### Metrics
```bash
GET /metrics  # Prometheus format
```

## Dependencies

### External
- `nomt` - Nearly-Optimal Merkle Trie database
- `tokio` - Async runtime
- `axum` - HTTP server
- `tower` / `tower-http` - Middleware and CORS
- `tonic` / `prost` - gRPC server
- `serde` / `serde_json` - Serialization
- `sha2` - SHA-256 hashing
- `hex` - Hex encoding
- `tracing` / `tracing-subscriber` - Logging
- `thiserror` / `anyhow` - Error handling
- `prometheus` - Metrics
- `chrono` - Date/time handling
- `reqwest` - HTTP client for Substrate
- `fs2` - File locking for NFS

### Internal
- None (self-contained service)

## Architecture

```
┌─────────────────────────────────────────────┐
│           NOMT Sidecar Service              │
│  ┌─────────────┐      ┌─────────────────┐  │
│  │ HTTP Server │      │   gRPC Server   │  │
│  │   (8081)    │      │    (50051)      │  │
│  └──────┬──────┘      └─────────────────┘  │
│         │                                   │
│  ┌──────▼──────────────────────────────┐   │
│  │        StorageBackend               │   │
│  │  ┌─────────────┐ ┌────────────────┐ │   │
│  │  │Native(NOMT) │ │Legacy(JSON)    │ │   │
│  │  │  (NVMe)     │ │  (NFS)         │ │   │
│  │  └─────────────┘ └────────────────┘ │   │
│  └──────────────────────────────────────┘   │
│         │                                   │
│  ┌──────▼──────┐      ┌────────────────┐   │
│  │ Anchoring   │      │  Peer Sync     │   │
│  │ Service     │      │  (StatefulSet) │   │
│  └─────────────┘      └────────────────┘   │
└─────────────────────────────────────────────┘
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `NOMT_DATA_DIR` | `/data/nomt` | Data storage directory |
| `NOMT_HTTP_PORT` | `8081` | HTTP server port |
| `NOMT_GRPC_PORT` | `50051` | gRPC server port |
| `NOMT_NATIVE` | `false` | Enable native NOMT mode |
| `BLOCKCHAIN_API_URL` | `https://blockchain.freshcredit.com` | Substrate API |
| `ANCHORING_ENABLED` | `true` | Enable root anchoring |
| `ANCHOR_INTERVAL_STORES` | `10` | Anchor after N stores |
| `ANCHOR_INTERVAL_SECONDS` | `300` | Anchor interval |
| `PEER_SYNC_ENABLED` | `false` | Enable peer sync |
| `POD_NAME` | - | Kubernetes pod name |

## Storage Modes

### Native NOMT (Recommended for local disk)
- Uses io_uring (Linux only)
- Requires local NVMe SSD
- Best performance
- **Not compatible with NFS**

### Legacy JSON (NFS-compatible)
- JSON file-based storage
- Works with shared NFS/Filestore
- Automatic file locking
- Suitable for Kubernetes StatefulSets

## Testing

```bash
# Run sidecar tests
cargo test -p nomt-sidecar

# Run with native NOMT
NOMT_NATIVE=true cargo run -p nomt-sidecar
```

## Related Documentation

📁 [NOMT WASM](../nomt-wasm/) - Browser proof verification

📁 [Blockchain Client](../blockchain-client/) - Substrate integration

📁 [Architecture Overview](../../../docs/application/architecture/)
