---
title: "freshcredit-core-timing"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "core"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# freshcredit-core-timing

Timing abstractions and utilities for the FreshCredit platform.

## Purpose

Provides timing abstractions and utilities for the FreshCredit platform. All timing logic should use these abstractions rather than direct time operations. This enables testable time-dependent code through mock clocks and provides consistent utilities for TTL enforcement, retry strategies, timeouts, and business day calculations.

## Directory Structure

```
crates/core/timing/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    ├── lib.rs          # Main module exports
    ├── clock.rs        # Clock abstraction (SystemClock, MockClock)
    ├── calendar.rs     # Business day calendar
    ├── freshness.rs    # Data freshness validation
    ├── id.rs           # ID generation utilities
    ├── retry.rs        # Retry strategies with backoff
    ├── settlement.rs   # ACH settlement timing
    ├── timeout.rs      # Timeout enforcement
    └── ttl.rs          # TTL (Time-To-Live) enforcement
```

## Key Types/Traits

| Name | Type | Description |
|------|------|-------------|
| `Clock` | trait | Abstraction for time operations |
| `SystemClock` | struct | Production clock using actual time |
| `MockClock` | struct | Test clock with manual time control |
| `BusinessDayCalendar` | struct | Calendar for business day calculations |
| `FreshnessValidator` | struct | Validates data freshness |
| `FreshnessStatus` | enum | Fresh, Stale, or Expired |
| `RetryStrategy` | trait | Retry strategy abstraction |
| `ExponentialBackoff` | struct | Exponential backoff retry |
| `RetryExecutor` | struct | Executes with retry logic |
| `TimeoutEnforcer` | struct | Enforces async timeouts |
| `TtlConfig` | struct | TTL configuration |
| `TtlEnforcement` | struct | TTL enforcement handler |
| `SettlementCalculator` | struct | ACH settlement date calculator |
| `AchSettlementType` | enum | Same-day or standard ACH |

## Usage

### Clock Abstraction

```rust
use freshcredit_core_timing::{Clock, SystemClock, MockClock};
use chrono::{DateTime, Utc};

// Production code
let clock = SystemClock;
let now = clock.now();

// Testing code
let mock_time = Utc::now();
let mut mock_clock = MockClock::new(mock_time);
mock_clock.advance(chrono::Duration::hours(1));
```

### Retry with Exponential Backoff

```rust
use freshcredit_core_timing::{with_retry, ExponentialBackoff};

let result = with_retry(
    || async { /* operation */ },
    ExponentialBackoff::default(),
).await?;
```

### Business Day Calendar

```rust
use freshcredit_core_timing::BusinessDayCalendar;

let calendar = BusinessDayCalendar::new();
let settlement_date = calendar.add_business_days(start_date, 2);
```

### ACH Settlement

```rust
use freshcredit_core_timing::{SettlementCalculator, AchSettlementType};

let calculator = SettlementCalculator::new();
let settlement = calculator.calculate(
    initiation_date,
    AchSettlementType::SameDay,
)?;
```

### Timeout Enforcement

```rust
use freshcredit_core_timing::TimeoutEnforcer;

let enforcer = TimeoutEnforcer::new(Duration::from_secs(30));
let result = enforcer.enforce(async_operation).await?;
```

### ID Generation

```rust
use freshcredit_core_timing::{
    new_id, prefixed_id, session_id, transaction_id,
    request_id, workflow_id, report_id, payment_method_id,
};

let id = new_id();                    // Generic ID
let txn_id = transaction_id();        // txn_xxx
let sess_id = session_id();           // sess_xxx
```

## Dependencies

### External
- `chrono` - Date/time handling with serde
- `serde` - Serialization
- `thiserror` - Error derive macros
- `async-trait` - Async trait support
- `tokio` - Async runtime
- `rand` - Random number generation
- `tracing` - Logging
- `uuid` - UUID generation (v4)

### Internal
- None (foundation crate)

## Architecture

```
┌─────────────────────────────────────────────┐
│       freshcredit-core-timing               │
│  ┌─────────┐ ┌─────────┐ ┌──────────────┐  │
│  │  Clock  │ │  Retry  │ │   Calendar   │  │
│  │(System/ │ │(Backoff)│ │(Business Day)│  │
│  │  Mock)  │ │         │ │              │  │
│  └─────────┘ └─────────┘ └──────────────┘  │
│  ┌─────────┐ ┌─────────┐ ┌──────────────┐  │
│  │   TTL   │ │ Timeout │ │  Settlement  │  │
│  │Enforcer │ │Enforcer │ │  Calculator  │  │
│  └─────────┘ └─────────┘ └──────────────┘  │
└─────────────────────────────────────────────┘
```

## Testing

```bash
# Run all timing tests
cargo test -p freshcredit-core-timing

# Run with mock clock tests
cargo test -p freshcredit-core-timing mock
```

## Design Principles

1. **Testability**: Use `MockClock` for deterministic time-based tests
2. **Consistency**: All time operations go through `Clock` trait
3. **Safety**: TTL and timeout enforcement prevent resource leaks
4. **Accuracy**: Business day calculations respect holidays

## Related Documentation

📁 [FreshCredit Types](../types/) - Core domain types

📁 [FreshCredit Config](../config/) - Configuration management

📁 [Architecture Overview](../../../docs/reference/architecture/)
