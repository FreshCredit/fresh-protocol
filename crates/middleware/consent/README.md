---
title: "freshcredit-middleware-consent"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "middleware"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# freshcredit-middleware-consent

Consent management middleware for FreshCredit.

## Purpose

Implements jurisdiction-specific consent rules (GDPR, CCPA, FCRA) and ensures data access only occurs with valid, non-expired consent. Provides database-backed consent storage with automatic expiration enforcement and Axum middleware integration.

## Directory Structure

```
crates/middleware/consent/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    ├── lib.rs          # Module exports
    ├── libsql_storage.rs  # LibSQL storage
    ├── middleware.rs   # Axum middleware
    ├── rules.rs        # Consent rules by jurisdiction
    ├── storage.rs      | Storage trait
    ├── types.rs        # Consent types
    ├── validator.rs    # Consent validation
    └── worker.rs       # Cleanup worker
```

## Key Types

| Name | Type | Description |
|------|------|-------------|
| `ConsentMiddleware` | struct | Axum middleware |
| `LibSqlConsentStorage` | struct | LibSQL storage |
| `Consent` | struct | Consent record |
| `ConsentType` | enum | Types of consent |
| `Jurisdiction` | enum | GDPR, CCPA, US |
| `ConsentValidator` | struct | Validation logic |
| `ConsentRules` | struct | Jurisdiction rules |

## Consent Types

| Type | US Duration | GDPR Duration | Description |
|------|-------------|---------------|-------------|
| Financial Data | 90 days | 12 months | Plaid access |
| LinkedIn Data | 30 days | 12 months | Professional data |
| Credit Report | 2 years | 2 years | FCRA compliance |
| Report Sharing | 1 year | 1 year | Third-party sharing |
| Marketing | 1 year | Until withdrawn | Marketing comms |
| Analytics | No expiry | 1 year | Usage analytics |

## Integration

```rust
use freshcredit_middleware_consent::{
    ConsentMiddleware, ConsentRequirement, LibSqlConsentStorage,
    ConsentType, Jurisdiction, consent_middleware,
};
use axum::{Router, routing::get};
use std::sync::Arc;

async fn example() {
    // Create consent storage
    let conn = Arc::new(/* LibSQL connection */);
    let storage = Arc::new(LibSqlConsentStorage::new(conn));
    
    // Create consent middleware
    let middleware = Arc::new(ConsentMiddleware::new(
        storage,
        Jurisdiction::US,
    ));
    
    // Define consent requirement
    let requirement = ConsentRequirement::single(
        ConsentType::FinancialDataAccess
    );
    
    // Add to Axum router
    let app = Router::new()
        .route("/plaid/data", get(handler))
        .layer(axum::middleware::from_fn(move |req, next| {
            consent_middleware(
                middleware.clone(),
                requirement.clone(),
                req,
                next,
            )
        }));
}
```

## Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `jurisdiction` | US | Default jurisdiction |
| `strict_mode` | false | Fail on missing consent |
| `auto_prompt` | true | Auto-request consent |

## Storage

```rust
use freshcredit_middleware_consent::{
    ConsentStorage, CreateConsentRequest, ConsentQuery
};

#[async_trait]
impl ConsentStorage for MyStorage {
    async fn create_consent(&self, req: CreateConsentRequest) -> Result<Consent>;
    async fn get_consent(&self, query: ConsentQuery) -> Result<Option<Consent>>;
    async fn revoke_consent(&self, id: &str) -> Result<()>;
    async fn cleanup_expired(&self) -> Result<u64>;
}
```

## Cleanup Worker

```rust
use freshcredit_middleware_consent::ConsentCleanupWorker;

let worker = ConsentCleanupWorker::new(storage, Duration::from_secs(86400));
worker.start().await?;
```

## Middleware Behavior

```
Request → ConsentMiddleware → Check Jurisdiction → Verify Consent
                                     ↓                    ↓
                             Get Requirement      Valid/Expired?
                                     ↓                    ↓
                             Check Storage         Denied/Proceed
                                     ↓
                             Return 403 or Continue
```

## Dependencies

### External
- `axum` - Web framework
- `tower` - Middleware
- `libsql` - Database storage
- `tokio` - Async runtime
- `async-trait` - Async traits
- `serde` / `serde_json` - Serialization
- `chrono` - Date/time
- `thiserror` / `anyhow` - Error handling
- `tracing` - Logging

### Internal
- `freshcredit-core-timing` - Clock abstraction

## Testing

```bash
cargo test -p freshcredit-middleware-consent
```

## Related Documentation

📁 [freshcredit-core-timing](../../core/timing/)

📁 [freshcredit-middleware-session](../session/)
