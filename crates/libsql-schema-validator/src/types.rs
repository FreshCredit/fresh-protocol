use serde::{Deserialize, Serialize};

/// Schema validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationResult {
    pub is_valid: bool,
    pub databases_checked: Vec<String>,
    pub issues: Vec<SchemaIssue>,
    pub warnings: Vec<SchemaWarning>,
    pub checked_at: String,
    pub summary: ValidationSummary,
}

/// Schema validation issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaIssue {
    pub severity: IssueSeverity,
    pub database: String,
    pub issue_type: IssueType,
    pub description: String,
    pub affected_object: String,
}

/// Issue severity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssueSeverity {
    Critical, // Schema drift that breaks functionality
    High,     // Missing tables or columns
    Medium,   // Missing indexes
    Low,      // Minor differences
}

/// Issue type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueType {
    MissingTable,
    ExtraTable,
    MissingColumn,
    ExtraColumn,
    ColumnTypeMismatch,
    MissingIndex,
    ExtraIndex,
    MissingForeignKey,
    ExtraForeignKey,
}

/// Schema warning (non-critical)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaWarning {
    pub database: String,
    pub warning_type: String,
    pub description: String,
}

/// Validation summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_tables_checked: usize,
    pub total_columns_checked: usize,
    pub total_indexes_checked: usize,
    pub critical_issues: usize,
    pub high_issues: usize,
    pub medium_issues: usize,
    pub low_issues: usize,
    pub warnings: usize,
}

/// Table schema information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
}

/// Column information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
    pub default_value: Option<String>,
    pub primary_key: bool,
}

/// Index information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

/// Foreign key information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForeignKeyInfo {
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
    pub on_delete: Option<String>,
}

/// Schema validator
pub struct SchemaValidator {
    pub(crate) staging_connection: Option<libsql::Connection>,
    pub(crate) local_connection: Option<libsql::Connection>,
    pub(crate) cloud_connection: Option<libsql::Connection>,
}
