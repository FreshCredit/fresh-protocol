---
title: "freshcredit-libsql-local"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "database"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# freshcredit-libsql-local

Local SQLite database operations for FreshCredit.

## Purpose

Implements the unified database schema for FreshCredit using local SQLite. Contains 136 tables supporting user profiles, financial data, AI features, payments, blockchain integration, and more. Provides CRUD operations organized by domain and includes test utilities with in-memory mode.

## Directory Structure

```
crates/db/libsql/local/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    ├── lib.rs          # LocalClient definition
    ├── types.rs        # UserProfile, Preferences, etc.
    ├── schema/         # Schema definitions by domain
    │   ├── mod.rs
    │   ├── core.rs
    │   ├── financial.rs
    │   └── ...
    └── operations/     # CRUD operations by domain
        ├── mod.rs
        ├── user.rs
        ├── accounts.rs
        └── ...
```

## Key Types

| Name | Type | Description |
|------|------|-------------|
| `LocalClient` | struct | Local SQLite database client |
| `UserProfile` | struct | User profile data |
| `UserPreferences` | struct | User preference settings |
| `AiConversation` | struct | AI chat conversation |
| `WorkflowRecord` | struct | Workflow automation record |
| `WebhookEvent` | struct | Webhook event data |

## Public API

```rust
impl LocalClient {
    /// Create a new local client
    pub async fn new(database_path: &str) -> Result<Self>;
    
    /// Create in-memory client for testing
    #[cfg(any(test, feature = "test-utils"))]
    pub async fn new_in_memory() -> Result<Self>;
    
    /// Initialize database schema (136 tables)
    pub async fn initialize_schema(&self) -> Result<()>;
    
    /// Execute a raw SQL query
    pub async fn query(&self, sql: &str, params: Vec<Value>) -> Result<Rows>;
    
    /// Execute a raw SQL statement
    pub async fn execute(&self, sql: &str, params: Vec<Value>) -> Result<u64>;
}
```

## Usage

### Basic Example

```rust
use freshcredit_libsql_local::LocalClient;

// Create local database client
let client = LocalClient::new("./data/freshcredit.db").await?;

// Initialize schema
client.initialize_schema().await?;

// Query data
let mut rows = client.query(
    "SELECT * FROM user_profile WHERE email = ?",
    vec![libsql::Value::Text("user@example.com".to_string())]
).await?;
```

### Testing

```rust
use freshcredit_libsql_local::LocalClient;

// Create in-memory client for tests
let client = LocalClient::new_in_memory().await?;
client.initialize_schema().await?;
```

## Schema Overview

| Domain | Table Count | Key Tables |
|--------|-------------|------------|
| Core | 6 | user_profile, user_preferences, api_keys |
| Financial | 3 | accounts, transactions, balances |
| Identity | 2 | identity_verification, verified_credentials |
| AI | 7 | ai_conversations, ai_messages, uploaded_files |
| Payments | 11 | customers, payments, crypto_wallets |
| Reports | 11 | reports, scores, offers |
| Security | 5 | sessions, user_devices, ip_blocks |
| ... | ... | ... |
| **Total** | **136** | See source for full list |

## Features

- `test-utils` - Enable in-memory database for testing

## Dependencies

### External
- `libsql` - libSQL driver
- `anyhow` - Error handling
- `tracing` - Logging
- `tokio` - Async runtime
- `serde` / `serde_json` - Serialization
- `uuid` - UUID generation
- `chrono` - Date/time
- `async-trait` - Async traits

### Internal
- `freshcredit-types` - Core domain types

## Testing

```bash
# Run all tests
cargo test -p freshcredit-libsql-local

# Run with test-utils feature
cargo test -p freshcredit-libsql-local --features test-utils
```

## Related Documentation

📁 [freshcredit-libsql-cloud](../cloud/)

📁 [freshcredit-db-migrations](../../migrations/)

📁 [freshcredit-types](../../../core/types/)
