---
title: "freshcredit-db-migrations"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "database"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# freshcredit-db-migrations

Versioned database migration system for FreshCredit.

## Purpose

Manages database schema evolution across environments. Tracks applied migrations in a `schema_migrations` table, applies migrations in version order, generates checksums to detect modifications, and supports rollback via down migrations. Migration files follow the `{version}_{name}.up.sql` / `{version}_{name}.down.sql` naming convention.

## Directory Structure

```
crates/db/migrations/
├── README.md           # This file
├── Cargo.toml          # Crate manifest
└── src/
    ├── lib.rs          # Migration runner library
    └── bin/
        └── run_migrations.rs  # CLI executable
```

## Key Types

| Name | Type | Description |
|------|------|-------------|
| `Migration` | struct | Represents a migration to apply |
| `AppliedMigration` | struct | Record of applied migration |
| `MigrationRunner` | struct | Main migration runner |

## Usage

### Library Usage

```rust
use freshcredit_db_migrations::MigrationRunner;
use std::path::Path;

// Create runner with connection
let runner = MigrationRunner::new(connection);

// Load migrations from directory
runner.load_migrations_from_dir(Path::new("migrations"))?;

// Run pending migrations
let applied = runner.run_pending_migrations().await?;
```

### CLI Usage

```bash
# Run migrations
cargo run -p freshcredit-db-migrations --bin run-migrations

# Or directly
cargo run --bin run-migrations
```

## Migration File Format

```
migrations/
├── 001_initial_schema.up.sql
├── 001_initial_schema.down.sql
├── 002_add_indexes.up.sql
├── 002_add_indexes.down.sql
└── ...
```

## Dependencies

### External
- `libsql` - Database driver
- `anyhow` - Error handling
- `serde` - Serialization
- `serde_json` - JSON handling
- `tracing` - Logging
- `chrono` - Date/time
- `thiserror` - Error types
- `tokio` - Async runtime
- `sha2` - Checksum hashing
- `hex` - Hex encoding

### Internal
- None

## Testing

```bash
cargo test -p freshcredit-db-migrations
```

## Related Documentation

📁 [freshcredit-libsql-local](../libsql/local/)

📁 [freshcredit-libsql-cloud](../libsql/cloud/)
