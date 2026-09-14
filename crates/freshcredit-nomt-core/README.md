# NOMT Core

**Version:** 0.1.0  
**Status:** Active  
**Purpose:** Core types and utilities for NOMT (Nearly Optimal Merkle Trie) proofs

---

## Overview

This crate provides the unified `NomtProof` type and verification logic that replaces 4 different implementations previously scattered across the codebase.

## Unified Proof Structure

```rust
pub struct NomtProof {
    /// Hash of the leaf node (the stored data)
    pub leaf_hash: String,
    /// Sibling hashes along the path to root
    pub siblings: Vec<String>,
    /// Path direction at each level (true = right, false = left)
    pub path: Vec<bool>,
    /// Root hash this proof verifies against
    pub root: String,
}

impl NomtProof {
    /// Verify the proof
    pub fn verify(&self) -> Result<bool, ProofError> {
        // Merkle proof verification
    }
}
```

## Problem Solved

Before this crate, there were 4 different `NomtProof` implementations:

1. `apps/engine/src/ports/nomt.rs` - Incompatible format
2. `crates/nomt-sidecar/src/types.rs` - Source of truth
3. `crates/nomt-wasm/src/lib.rs` - Vec<u8> variant
4. `crates/blockchain-client/src/types.rs` - Opaque blob format

Now all crates use this single unified implementation.

## Usage

```rust
use nomt_core::{NomtProof, compute_hash};

// Create a proof
let proof = NomtProof::new(
    "abc123",
    vec!["sibling1".to_string(), "sibling2".to_string()],
    vec![true, false],
    "root123",
);

// Verify it
assert!(proof.verify().unwrap());
```

## Binary Format

For WASM/JS compatibility:

```rust
let binary = proof.to_binary(); // NomtProofBinary
let recovered = binary.to_string_format()?;
```

## Related Crates

- `nomt-server` - Server-side storage implementations
- `nomt-client` - HTTP client for NOMT API
- `nomt-wasm` - WASM bindings (uses nomt-core)

## License

MIT OR Apache-2.0
