---
title: "freshcredit-middleware-idempotency"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "middleware"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# freshcredit-middleware-idempotency

Idempotency key enforcement middleware for FreshCredit API endpoints.

## Purpose

Ensures that duplicate requests with the same idempotency key return the same response, preventing duplicate operations like double payments, duplicate reports, or duplicate data modifications. Essential for safe retry handling in distributed systems.

## Directory Structure

```
crates/middleware/idempotency/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    ├── lib.rs          # Module exports
    ├── config.rs       # Idempotency configuration
    ├── middleware.rs   # Axum middleware layer
    └── store.rs        | Idempotency storage
```

## Key Types

| Name | Type | Description |
|------|------|-------------|
| `IdempotencyLayer` | struct | Axum middleware layer |
| `IdempotencyConfig` | struct | Configuration |
| `IdempotencyStore` | trait | Storage abstraction |
| `StoredResponse` | struct | Cached response |
| `IdempotencyError` | enum | Error types |

## Integration

```rust
use freshcredit_middleware_idempotency::{IdempotencyLayer, IdempotencyConfig};
use axum::{Router, routing::post};

// Create idempotency layer
let config = IdempotencyConfig::default();
let store = Arc::new(MyIdempotencyStore::new());
let idempotency_layer = IdempotencyLayer::new(config, store);

// Apply to Axum router
let app = Router::new()
    .route("/payments", post(create_payment))
    .layer(idempotency_layer);
```

## Client Usage

```http
POST /payments
Idempotency-Key: 550e8400-e29b-41d4-a716-446655440000

{ "amount": 100, "currency": "USD" }
```

## Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `key_header` | "Idempotency-Key" | Header name |
| `ttl_seconds` | 86400 | Cache TTL (24 hours) |
| `max_key_length` | 256 | Max key length |

## Storage Implementation

```rust
use freshcredit_middleware_idempotency::{
    IdempotencyStore, StoredResponse
};

#[async_trait]
impl IdempotencyStore for MyStore {
    async fn get(&self, key: &str) -> Result<Option<StoredResponse>>;
    async fn store(&self, key: &str, response: &StoredResponse, ttl: Duration) -> Result<()>;
}
```

## Middleware Behavior

```
Request → IdempotencyLayer → Extract Key → Check Store
                                 ↓              ↓
                             No Key?       Found?
                                 ↓              ↓
                             Continue    Return Cached
                                 ↓              ↓
                             Handler → Store Response
```

## Dependencies

### External
- `axum` - Web framework
- `tokio` - Async runtime
- `tracing` - Logging
- `thiserror` - Error types
- `serde` / `serde_json` - Serialization
- `chrono` - Date/time
- `dashmap` - In-memory storage
- `uuid` - UUID handling
- `bytes` - Byte handling
- `http-body-util` - Body utilities

### Internal
- `freshcredit-core-timing` - Clock abstraction

## Testing

```bash
cargo test -p freshcredit-middleware-idempotency
```

## Related Documentation

📁 [freshcredit-core-timing](../../core/timing/)

📁 [freshcredit-middleware-rate-limit](../rate_limit/)
