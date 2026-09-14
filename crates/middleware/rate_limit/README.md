---
title: "freshcredit-middleware-rate-limit"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "middleware"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# freshcredit-middleware-rate-limit

Rate limiting middleware for FreshCredit API endpoints.

## Purpose

Provides two types of rate limiting: HTTP Middleware Rate Limiting with sliding window algorithm (tier-based limits for Anonymous, Consumer, and Provider) and Operation-Specific Rate Limiting with token bucket algorithm for fine-grained control of expensive operations like Plaid sync and report generation.

## Directory Structure

```
crates/middleware/rate_limit/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    ├── lib.rs          # Module exports
    ├── config.rs       # Rate limit configuration
    ├── limiter.rs      # Rate limiter implementation
    ├── middleware.rs   # Axum middleware layer
    └── operation.rs    # Operation-specific limiting
```

## Key Types

| Name | Type | Description |
|------|------|-------------|
| `RateLimitLayer` | struct | Axum middleware layer |
| `RateLimitConfig` | struct | Configuration |
| `RateLimitTier` | enum | Anonymous, Consumer, Provider |
| `RateLimiter` | struct | Sliding window limiter |
| `OperationRateLimiterService` | struct | Token bucket limiter |
| `RateLimitedOperation` | enum | Operations to limit |

## Integration

```rust
use freshcredit_middleware_rate_limit::{RateLimitLayer, RateLimitConfig};
use freshcredit_core_timing::Clock;
use std::sync::Arc;

// Create rate limit layer
let config = RateLimitConfig::anonymous();
let clock: Arc<dyn Clock> = Arc::new(RealClock::new());
let rate_limit_layer = RateLimitLayer::new(config, clock);

// Apply to Axum router
let app = Router::new()
    .route("/api/*", get(handler))
    .layer(rate_limit_layer);
```

## Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `requests_per_window` | tier-based | Requests allowed per window |
| `window_duration` | 60s | Time window for counting |
| `burst_size` | 10 | Burst allowance |

## Rate Limit Tiers

| Tier | Requests/Min | Description |
|------|--------------|-------------|
| Anonymous | 200 | Unauthenticated requests |
| Consumer | 300 | Standard user requests |
| Provider | 600 | Provider/vendor requests |

## Operation-Specific Limits

```rust
use freshcredit_middleware_rate_limit::{
    OperationRateLimiterService, RateLimitedOperation
};

let limiter = Arc::new(OperationRateLimiterService::new(clock));

// In route handler:
limiter.check_operation(
    user_id,
    RateLimitedOperation::ReportGeneration,
    RateLimitTier::Consumer
)?;
```

## Middleware Behavior

```
Request → RateLimitLayer → Check Tier → Check Window → Handler
                              ↓              ↓
                          Rejected?      Rejected?
                              ↓              ↓
                        Return 429     Return 429
```

## Dependencies

### External
- `axum` - Web framework
- `tokio` - Async runtime
- `tracing` - Logging
- `thiserror` - Error types
- `serde` - Serialization
- `chrono` - Date/time
- `dashmap` - Concurrent hash map

### Internal
- `freshcredit-core-timing` - Clock abstraction

## Testing

```bash
cargo test -p freshcredit-middleware-rate-limit
```

## Related Documentation

📁 [freshcredit-core-timing](../../core/timing/)

📁 [freshcredit-middleware-session](../session/)
