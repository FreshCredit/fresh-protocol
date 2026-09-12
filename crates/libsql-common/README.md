---
title: "freshcredit-libsql-common"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "database"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# freshcredit-libsql-common

Shared utilities for libSQL/Turso database connections.

## Purpose

Provides centralized database utilities for per-user Turso cloud databases. Handles URL construction with consistent sanitization, connection factory with retry logic, and database connection configuration. Used by both local and cloud database clients.

## Directory Structure

```
crates/db/libsql/common/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    ├── lib.rs          # Module exports
    ├── connection_factory.rs  # Retry logic
    └── url_builder.rs  # Turso URL construction
```

## Key Types

| Name | Type | Description |
|------|------|-------------|
| `TursoUrlBuilder` | struct | Per-user database URL generator |
| `RetryConfig` | struct | Connection retry configuration |
| `with_retry` | function | Retry wrapper for operations |

## Usage

### URL Construction

```rust
use freshcredit_libsql_common::TursoUrlBuilder;

let builder = TursoUrlBuilder::new("devonshigaki");
let url = builder.user_database_url("user@example.com");
// Returns: libsql://user-user-example-com-{org}.aws-us-west-2.turso.io
```

### Connection Retry

```rust
use freshcredit_libsql_common::{RetryConfig, with_retry};

let config = RetryConfig::default();
let client = with_retry(&config, || async {
    CloudClient::new(&url, &token).await
}).await?;
```

## Dependencies

### External
- `thiserror` - Error handling
- `tracing` - Logging
- `tokio` - Async runtime
- `rand` - Random number generation

### Internal
- None

## Architecture

```
┌─────────────────────────────────────┐
│  freshcredit-libsql-common          │
│  ┌──────────────┐ ┌──────────────┐  │
│  │TursoUrlBuilder│ │ RetryConfig  │  │
│  └──────────────┘ └──────────────┘  │
└─────────────────────────────────────┘
           ▲                ▲
           │                │
    ┌──────┘                └──────┐
    ▼                              ▼
┌─────────────────┐      ┌─────────────────┐
│libsql-local     │      │libsql-cloud     │
└─────────────────┘      └─────────────────┘
```

## Testing

```bash
cargo test -p freshcredit-libsql-common
```

## Related Documentation

📁 [freshcredit-libsql-local](../local/)

📁 [freshcredit-libsql-cloud](../cloud/)
