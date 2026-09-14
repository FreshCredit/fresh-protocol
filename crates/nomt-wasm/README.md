---
title: "nomt-wasm"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "blockchain"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# nomt-wasm

NOMT proof verification for browser via WebAssembly.

## Purpose

Provides WebAssembly bindings for verifying NOMT (Nearly-Optimal Merkle Trie) Binary Merkle Trie proofs in the browser. Works alongside smoldot (Patricia Trie) as an ADDITIONAL verification method, not a replacement. Enables trustless cryptographic verification of off-chain data without trusting the server.

## Directory Structure

```
crates/nomt-wasm/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    └── lib.rs          # WASM bindings and proof verification
```

## Key Types/Traits

| Name | Type | Description |
|------|------|-------------|
| `NomtProof` | struct | Binary Merkle Trie proof structure |
| `verify_nomt_proof()` | function | Main verification function |
| `sha256_hash()` | function | SHA-256 utility |
| `get_version()` | function | WASM module version |

## Usage

### Rust (WebAssembly)

```rust
use nomt_wasm::NomtProof;

// Create proof from components
let proof = NomtProof::new(
    leaf_hash_hex,
    siblings_hex,  // Vec<JsValue> of hex strings
    path,          // Vec<u8> (0 = left, non-zero = right)
    root_hex,
)?;

// Verify against expected root
let is_valid = proof.verify(expected_root_hex)?;
```

### JavaScript/TypeScript

```javascript
// Load the WASM module
import init, { verify_nomt_proof, sha256_hash } from './nomt_wasm.js';

await init();

// Verify a proof
const isValid = verify_nomt_proof(
    leafHashHex,      // Hex string
    siblingsHex,      // Array of hex strings
    path,             // Uint8Array (0 = left, 1 = right)
    expectedRootHex   // Hex string
);

// Compute SHA-256 hash
const hash = sha256_hash(data);  // Returns hex string
```

### Proof Structure

```javascript
{
  leaf_hash: "abc123...",      // 32 bytes as hex
  siblings: ["def456...", ...], // Sibling hashes along path
  path: [0, 1, 0, ...],        // Direction at each level
  root: "789abc..."            // Root hash this proof verifies
}
```

## Building

```bash
# Install wasm-pack
cargo install wasm-pack

# Build for web target
wasm-pack build --target web

# Build for bundler (webpack/rollup)
wasm-pack build --target bundler

# Build for Node.js
wasm-pack build --target nodejs
```

## Dependencies

### External
- `nomt-core` - NOMT Binary Merkle Trie core (no_std compatible)
- `wasm-bindgen` - WASM bindings
- `serde` - Serialization for JS interop
- `serde-wasm-bindgen` - Serde integration
- `sha2` - SHA-256 hashing (no_std)
- `hex` - Hex encoding (alloc)
- `web-sys` - JavaScript console logging

### Internal
- None (self-contained WASM crate)

## Architecture

```
Server (NOMT Sidecar)
    ↓
Generates NOMT Proof
    ↓
Browser receives Data + Proof
    ↓
nomt-wasm verifies proof
    ↓
Trustless verification complete
```

## Verification Flow

```
Leaf Hash
    ↓
Hash with Sibling (at each level)
    ↓
Following Path directions
    ↓
Reconstructed Root
    ↓
Compare with Expected Root
```

## NOMT vs Patricia Trie

| Feature | NOMT | Patricia Trie (smoldot) |
|---------|------|------------------------|
| Children per node | 2 (Binary) | 16 |
| Path compression | No | Yes |
| Use case | Off-chain proofs | On-chain state |
| Relationship | Additional | Primary |

## Testing

```bash
# Run WASM tests (requires wasm-pack)
wasm-pack test --headless --firefox

# Run unit tests
cargo test -p nomt-wasm
```

## Related Documentation

📁 [NOMT Sidecar](../nomt-sidecar/) - Server-side NOMT storage

📁 [Blockchain Client](../blockchain-client/) - Substrate integration

📁 [Architecture Overview](../../../docs/application/architecture/)
