# NOMT Server

**Version:** 0.1.0  
**Status:** Active  
**Purpose:** Server-side storage implementations for NOMT proofs

---

## Overview

This crate provides unified storage backends for the NOMT system, supporting both legacy JSON-based storage and native NOMT library storage.

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `native` | Native NOMT library storage | No (disabled) |
| `legacy` | Legacy JSON-based storage (NFS-compatible) | ✅ Yes |

## Storage Backends

### Legacy Storage (Default)
```rust
use nomt_server::{UnifiedStorage, LegacyStorage};

let storage = LegacyStorage::new("/data/nomt")?;
let unified = UnifiedStorage::new(Arc::new(RwLock::new(storage)));
```

### Native Storage (Requires `native` feature)
```rust
use nomt_server::{UnifiedStorage, NativeStorage};

let storage = NativeStorage::new("/data/nomt")?;
let unified = UnifiedStorage::new(Arc::new(RwLock::new(storage)));
```

## Unified Storage Trait

```rust
#[async_trait]
pub trait Storage: Send + Sync {
    async fn store(&self, request: StoreRequest) -> Result<StoreResponse, ServerError>;
    async fn verify(&self, request: VerifyRequest) -> Result<VerifyResponse, ServerError>;
    async fn stats(&self) -> Result<StorageStats, ServerError>;
}
```

## Related Crates

- `nomt-core` - Core types and proof verification
- `nomt-client` - HTTP client for this server
- `nomt-wasm` - Browser-side proof verification

## License

MIT OR Apache-2.0
