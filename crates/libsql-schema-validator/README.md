---
title: "freshcredit-libsql-schema-validator"
date: "2026-03-05"
last_reviewed: "2026-03-05"
status: "stable"
category: "database"
---
<!-- DOC-002: Stable -->
<!-- DOC-007: Active -->
<!-- Last Updated: 2026-03-05 -->

# freshcredit-libsql-schema-validator

Schema validation and drift detection for LibSQL databases.

## Purpose

Provides tools to detect schema drift between staging, local, and cloud databases. Ensures all three database types maintain consistent schema structure. Validates schema against migration files and detects missing tables, columns, indexes, and foreign keys. Critical for maintaining data integrity across development, staging, and production environments.

## Directory Structure

```
crates/db/libsql/schema-validator/
├── README.md                # This file
├── Cargo.toml               # Crate manifest
└── src/
    ├── lib.rs               # Main validation library
    ├── bin/
    │   └── validate_schema.rs # CLI binary
    └── tests/
        └── schema_sync_test.rs # Test utilities
```

## Key Types/Traits

| Name | Type | Description |
|------|------|-------------|
| `SchemaValidator` | struct | Main validator for database schemas |
| `SchemaValidationResult` | struct | Result of validation across databases |
| `SchemaIssue` | struct | Detected schema issue |
| `SchemaWarning` | struct | Non-critical warning |
| `ValidationSummary` | struct | Summary of validation results |
| `TableSchema` | struct | Table schema information |
| `ColumnInfo` | struct | Column details |
| `IndexInfo` | struct | Index details |
| `ForeignKeyInfo` | struct | Foreign key details |
| `IssueSeverity` | enum | Critical, High, Medium, Low |
| `IssueType` | enum | MissingTable, MissingColumn, etc. |

## Usage

### CLI Usage

```bash
# Run the validator binary
cargo run --bin validate-schema

# Run with specific databases
DATABASE_URL=libsql://... cargo run --bin validate-schema
```

### Library Usage

```rust
use freshcredit_libsql_schema_validator::SchemaValidator;

// Create validator
let validator = SchemaValidator::new()
    .with_staging(staging_conn)
    .with_local(local_conn)
    .with_cloud(cloud_conn);

// Run validation
let result = validator.validate().await?;

// Check results
if result.is_valid {
    println!("All schemas are synchronized!");
} else {
    for issue in &result.issues {
        eprintln!("{}: {} - {}", 
            issue.severity, 
            issue.database, 
            issue.description
        );
    }
}

// Access summary
println!("Checked {} tables", result.summary.total_tables_checked);
```

### Schema Comparison

```rust
use freshcredit_libsql_schema_validator::{
    parse_sql_schema, compare_schemas, load_migration_schema
};

// Load schema from migration file
let migration_schema = load_migration_schema("migrations/unified_schema.sql")?;

// Parse database schema
let db_schema = parse_sql_schema(&sql_text)?;

// Compare
let issues = compare_schemas(&migration_schema, &db_schema)?;
```

## Dependencies

### External
- `libsql` - LibSQL client
- `anyhow` - Error handling
- `serde` / `serde_json` - Serialization
- `tracing` / `tracing-subscriber` - Logging
- `chrono` - Date/time handling
- `thiserror` - Error derive macros
- `tokio` - Async runtime
- `regex` - SQL parsing

### Internal
- None (database utility crate)

## Architecture

```
┌─────────────────────────────────────────────┐
│         SchemaValidator                     │
│  ┌─────────┐ ┌─────────┐ ┌────────────────┐│
│  │ Staging │ │  Local  │ │     Cloud      ││
│  │   DB    │ │   DB    │ │      DB        ││
│  └────┬────┘ └────┬────┘ └───────┬────────┘│
│       └─────────────┴─────────────┘         │
│                   ↓                         │
│         extract_schema()                    │
│                   ↓                         │
│         compare_schemas()                   │
│                   ↓                         │
│         SchemaValidationResult              │
└─────────────────────────────────────────────┘
```

## Issue Severity Levels

| Level | Description | Example |
|-------|-------------|---------|
| Critical | Breaks functionality | Missing primary key table |
| High | Missing required objects | Missing table or column |
| Medium | Performance impact | Missing index |
| Low | Minor differences | Extra column |

## Validation Checklist

- [ ] All tables exist in all databases
- [ ] Columns match (name, type, constraints)
- [ ] Indexes are present
- [ ] Foreign keys are defined
- [ ] No unexpected extra tables/columns

## Environment Variables

| Variable | Description |
|----------|-------------|
| `DATABASE_URL` | Primary database URL |
| `STAGING_DATABASE_URL` | Staging database URL |
| `CLOUD_DATABASE_URL` | Cloud/Turso database URL |

## Testing

```bash
# Run validator tests
cargo test -p freshcredit-libsql-schema-validator

# Run with schema sync tests
cargo test -p freshcredit-libsql-schema-validator --features schema-sync-tests

# Run the binary
cargo run --bin validate-schema
```

## Related Documentation

📁 [LibSQL Local](../local/) - Local database client

📁 [LibSQL Cloud](../cloud/) - Cloud database client

📁 [LibSQL Staging](../staging/) - Staging database

📁 [Database Migrations](../../migrations/) - Migration system

📁 [Architecture Overview](../../../../docs/application/architecture/)
