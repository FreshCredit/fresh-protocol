---
title: "freshcredit-types"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "core"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# freshcredit-types

Core domain types shared across the entire FreshCredit system.

## Purpose

Provides foundational data structures that serve as the shared language for all FreshCredit components. This crate defines user identities, financial accounts, transactions, payments, and error types used throughout the platform. All types implement serde serialization for API compatibility and include RFC 7807 Problem Details for standardized HTTP error responses.

## Directory Structure

```
crates/core/types/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    └── lib.rs          # Type definitions
```

## Key Types

| Name | Type | Description |
|------|------|-------------|
| `UserId` | type alias | User identifier (String) |
| `Account` | struct | Financial account information |
| `Transaction` | struct | Financial transaction record |
| `FinancialReport` | struct | Aggregated financial report data |
| `Payment` | struct | Payment information |
| `PaymentStatus` | enum | Payment status variants |
| `PaymentMethod` | struct | Payment method with token references |
| `UserProfile` | struct | User profile information |
| `ApiResponse<T>` | struct | Generic API response wrapper |
| `FreshCreditError` | enum | Error types for FreshCredit operations |
| `ProblemDetails` | struct | RFC 7807 Problem Details |
| `FreshCreditResult<T>` | type alias | Result type alias |

## Usage

### Basic Example

```rust
use freshcredit_types::{Account, AccountType, Transaction, ApiResponse, FreshCreditResult};

// Create an account
let account = Account {
    id: "acct_123".to_string(),
    user_id: "user_456".to_string(),
    account_type: AccountType::Checking,
    balance: Some(1000.50),
    currency: "USD".to_string(),
    institution_name: "Test Bank".to_string(),
    created_at: chrono::Utc::now(),
};

// Create a successful API response
let response = ApiResponse::success(account);
```

### Error Handling

```rust
use freshcredit_types::{FreshCreditError, ProblemDetails};

// Create a validation error
let err = FreshCreditError::ValidationError("Invalid input".to_string());

// Convert to RFC 7807 Problem Details
let problem: ProblemDetails = err.into();
```

## Dependencies

### External
- `serde` - Serialization framework
- `serde_json` - JSON serialization
- `chrono` - Date/time handling
- `uuid` - UUID generation
- `anyhow` - Error handling
- `thiserror` - Derive macros for errors

### Internal
- None (foundation crate)

## Architecture

```
┌─────────────────────────────────────────┐
│        freshcredit-types                │
│  ┌─────────┐ ┌─────────┐ ┌──────────┐  │
│  │ Account │ │Payment  │ │ UserId   │  │
│  │Transaction│ │ProblemDetails│ │     │  │
│  └─────────┘ └─────────┘ └──────────┘  │
└─────────────────────────────────────────┘
         ▲                ▲
         │                │
    All other crates depend on this
```

## Testing

```bash
cargo test -p freshcredit-types
```

## Related Documentation

📁 [Architecture Overview](../../../docs/application/architecture/)

📁 [freshcredit-config](../config/)
