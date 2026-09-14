---
title: "FreshCredit Blockchain Service"
date: "2026-02-16"
last_reviewed: "2026-03-05"
status: "stable"
category: "blockchain"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-02-16 -->

# FreshCredit Blockchain Service

Blockchain explorer and verification service for FreshCredit's Substrate-based blockchain.

## Purpose

The blockchain service provides:
- **Block Explorer** - Web interface for viewing blockchain data
- **Transaction Verification** - Verify data integrity via blockchain proofs
- **API Endpoints** - REST API for blockchain queries
- **Real-time Updates** - WebSocket connections for live block updates

## Architecture

```
apps/blockchain/
├── Cargo.toml           # Package manifest
├── src/
│   └── main.rs          # Service entry point
├── templates/           # Askama HTML templates
├── static/              # Static assets (CSS, JS)
├── blockchain-data/     # Local blockchain data storage
└── Dockerfile           # Container image definition
```

## Technology Stack

- **Runtime**: Tokio async runtime
- **Web Framework**: Axum (same as other FreshCredit services)
- **Templates**: Askama with Tailwind CSS
- **Blockchain Client**: Substrate subxt
- **Node.js**: Tailwind CSS compilation

## Quick Start

```bash
# Build the blockchain service
cargo build -p freshcredit-blockchain

# Run locally (requires blockchain node connection)
cargo run -p freshcredit-blockchain

# Run with custom configuration
BLOCKCHAIN_WS_URL=wss://testnet.freshcredit.com cargo run -p freshcredit-blockchain
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `BLOCKCHAIN_WS_URL` | `ws://localhost:9944` | Substrate node WebSocket URL |
| `PORT` | `8080` | HTTP server port |
| `RUST_LOG` | `info` | Logging level |

## Docker

```bash
# Build Docker image
docker build -t freshcredit-blockchain -f Dockerfile .

# Run container
docker run -p 8080:8080 -e BLOCKCHAIN_WS_URL=wss://testnet.freshcredit.com freshcredit-blockchain
```

## Related Documentation

- 📁 [Architecture Overview](../../docs/application/architecture/)
- 📁 [API Reference](../../docs/application/api/)
- 📁 [Compliance Controls](../../docs/compliance/controls/)
- [Blockchain Client](../../crates/blockchain-client/) - Rust client library
- [Testnet](https://testnet.freshcredit.com) - Public testnet environment

## Related Services

- [`apps/web/`](../web/) - Main web application
- [`apps/api/`](../api/) - REST API service
- [`crates/blockchain-client/`](../../crates/blockchain-client/) - Blockchain client crate

## Deployment

See [docs/deployment/](../../docs/deployment/) for deployment instructions.
