//! Schema validation and drift detection for LibSQL databases
//!
//! This module provides tools to detect schema drift between staging, local, and cloud databases.
//! It ensures all three database types maintain the same schema structure.
//!
//! ## Schema Sources
//! - Rust schema definitions: `crates/db/libsql/local/src/schema/*.rs`
//! - SQL migration file: `migrations/unified_schema.sql`
//! - Turso cloud database: `freshcredit-unified-schema-v1`
//!
//! ## Usage
//! Run the validator binary: `cargo run --bin validate-schema`
//! Or use the library programmatically in tests.

pub mod tests;
pub mod types;
pub mod validator;

pub use tests::schema_sync_test::{
    compare_schemas, load_migration_schema, parse_sql_schema, SchemaColumn, TableDef,
};
pub use types::{
    ColumnInfo, ForeignKeyInfo, IndexInfo, IssueSeverity, IssueType, SchemaIssue,
    SchemaValidationResult, SchemaValidator, SchemaWarning, TableSchema, ValidationSummary,
};
