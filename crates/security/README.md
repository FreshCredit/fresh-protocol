---
title: "freshcredit-security"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "core"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# freshcredit-security

Cryptographic primitives and security utilities for FreshCredit.

## Purpose

Provides cryptographic utilities for data integrity and token security. Implements Blake2b hashing for data fingerprinting and AES-256-GCM encryption for sensitive token storage. Includes utilities for key generation and time-based security operations.

## Directory Structure

```
crates/core/security/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    ├── lib.rs          # Main module with hashing
    ├── clock.rs        # Time-based security utilities
    ├── encryption.rs   # Token encryption (AES-256-GCM)
    └── bin/
        └── fc_security.rs  # CLI utility
```

## Key Types/Functions

| Name | Type | Description |
|------|------|-------------|
| `blake2_256_hex` | function | Generate Blake2b-256 hash |
| `verify_hash` | function | Validate data integrity |
| `TokenEncryptor` | struct | AES-256-GCM encryptor |
| `encrypt_token` | function | Encrypt a token |
| `decrypt_token` | function | Decrypt a token |
| `generate_hex_key` | function | Generate encryption key |
| `EncryptionConfig` | struct | Encryption configuration |

## Usage

### Hashing

```rust
use freshcredit_security::{blake2_256_hex, verify_hash};
use serde::Serialize;

#[derive(Serialize)]
struct Data {
    name: String,
    value: i32,
}

let data = Data { name: "test".to_string(), value: 42 };
let hash = blake2_256_hex(&data)?;
assert!(verify_hash(&data, &hash)?);
```

### Token Encryption

```rust
use freshcredit_security::{encrypt_token, decrypt_token};

// Encrypt a sensitive token before storage
let encrypted = encrypt_token("my-access-token")?;

// Decrypt when retrieving
let decrypted = decrypt_token(&encrypted)?;
```

### CLI Usage

```bash
# Generate a new encryption key
cargo run -p freshcredit-security --bin fc-security -- generate-key
```

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `FRESHCREDIT_ENCRYPTION_ENABLED` | No | Set to "false" to disable (testing only) |
| `FRESHCREDIT_TOKEN_ENCRYPTION_KEY` | Yes* | 64-character hex encryption key |

*Required when encryption is enabled

## Dependencies

### External
- `blake2` - Hashing algorithm
- `aes-gcm` - AES-GCM encryption
- `serde` - Serialization
- `rand` - Random number generation
- `base64` - Base64 encoding
- `hex` - Hex encoding
- `chrono` - Date/time
- `anyhow` - Error handling
- `tracing` - Logging

### Internal
- None (foundation crate)

## Testing

```bash
cargo test -p freshcredit-security
```

## Related Documentation

📁 [Architecture Overview](../../../docs/application/architecture/)
