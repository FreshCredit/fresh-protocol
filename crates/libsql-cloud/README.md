---
title: "freshcredit-libsql-cloud"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "database"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# freshcredit-libsql-cloud

Cloud LibSQL (Turso) database operations for FreshCredit.

## Purpose

Manages per-user Turso cloud databases with unified schema synchronization. Provides data sync capabilities for financial reports, accounts, transactions, and user profiles. Used for cloud backup and multi-device synchronization.

## Directory Structure

```
crates/db/libsql/cloud/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    └── lib.rs          # CloudClient definition
```

## Key Types

| Name | Type | Description |
|------|------|-------------|
| `CloudClient` | struct | Turso cloud database client |

## Public API

```rust
impl CloudClient {
    /// Create a new cloud client
    pub async fn new(database_url: &str, auth_token: &str) -> Result<Self>;
    
    /// Initialize cloud database schema
    pub async fn initialize_schema(&self) -> Result<()>;
    
    /// Sync financial report to cloud
    pub async fn sync_financial_report(&self, report: &FinancialReport) -> FreshCreditResult<()>;
    
    /// Get financial report from cloud
    pub async fn get_financial_report(&self, user_id: &UserId) -> FreshCreditResult<Option<FinancialReport>>;
    
    /// Sync account data to cloud
    pub async fn sync_account(&self, account: &Account) -> FreshCreditResult<()>;
    
    /// Sync transaction data to cloud
    pub async fn sync_transaction(&self, transaction: &Transaction) -> FreshCreditResult<()>;
    
    /// Sync user profile to cloud
    pub async fn sync_user_profile(&self, profile: &UserProfile) -> FreshCreditResult<()>;
    
    /// Get user profile from cloud
    pub async fn get_user_profile(&self, user_email: &str) -> FreshCreditResult<Option<UserProfile>>;
    
    /// Get user accounts from cloud
    pub async fn get_user_accounts(&self, user_id: &str) -> FreshCreditResult<Vec<Account>>;
    
    /// Get user transactions from cloud
    pub async fn get_user_transactions(&self, user_id: &str) -> FreshCreditResult<Vec<Transaction>>;
    
    /// Get Plaid access token
    pub async fn get_plaid_access_token(&self, user_id: &str) -> FreshCreditResult<Option<String>>;
}
```

## Usage

### Basic Example

```rust
use freshcredit_libsql_cloud::CloudClient;

// Create cloud client
let client = CloudClient::new(
    "libsql://user-db.turso.io",
    "auth-token-here"
).await?;

// Initialize schema
client.initialize_schema().await?;

// Sync a financial report
client.sync_financial_report(&report).await?;
```

## Architecture

```
┌─────────────────────────────────────────┐
│         User Browser/Device             │
│    ┌─────────────────────────┐         │
│    │   freshcredit-libsql-local│        │
│    │   (Primary - SQLite)     │        │
│    └─────────────┬───────────┘         │
│                  │ Sync                 │
└──────────────────┼─────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────┐
│        Per-User Turso Cloud             │
│    ┌─────────────────────────┐         │
│    │  freshcredit-libsql-cloud│        │
│    │  (Backup/Sync - Turso)   │        │
│    └─────────────────────────┘         │
└─────────────────────────────────────────┘
```

## Dependencies

### External
- `libsql` - libSQL driver
- `anyhow` - Error handling
- `tracing` - Logging
- `tokio` - Async runtime
- `serde_json` - JSON handling
- `uuid` - UUID generation
- `chrono` - Date/time

### Internal
- `freshcredit-types` - Core domain types
- `freshcredit-libsql-local` - Local DB types

## Testing

```bash
cargo test -p freshcredit-libsql-cloud
```

## Related Documentation

📁 [freshcredit-libsql-local](../local/)

📁 [freshcredit-types](../../../core/types/)
