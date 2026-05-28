use serde::{Deserialize, Serialize};

/// Schema validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationResult {
    /// Whether the schema is valid across all checked databases
    pub is_valid: bool,
    /// Names of databases that were checked
    pub databases_checked: Vec<String>,
    /// Schema issues found during validation
    pub issues: Vec<SchemaIssue>,
    /// Non-critical schema warnings
    pub warnings: Vec<SchemaWarning>,
    /// ISO 8601 timestamp of when validation occurred
    pub checked_at: String,
    /// Summary counts of issues and warnings
    pub summary: ValidationSummary,
}

/// Schema validation issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaIssue {
    /// Severity level of the issue
    pub severity: IssueSeverity,
    /// Name of the database where the issue was found
    pub database: String,
    /// Category of schema issue
    pub issue_type: IssueType,
    /// Human-readable description of the issue
    pub description: String,
    /// Name of the table or object affected
    pub affected_object: String,
}

/// Issue severity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IssueSeverity {
    /// Schema drift that breaks functionality
    Critical,
    /// Missing tables or columns
    High,
    /// Missing indexes
    Medium,
    /// Minor differences
    Low,
}

/// Issue type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IssueType {
    /// Table exists in reference but not in target
    MissingTable,
    /// Table exists in target but not in reference
    ExtraTable,
    /// Column exists in reference but not in target
    MissingColumn,
    /// Column exists in target but not in reference
    ExtraColumn,
    /// Same column name but different data types
    ColumnTypeMismatch,
    /// Index exists in reference but not in target
    MissingIndex,
    /// Index exists in target but not in reference
    ExtraIndex,
    /// Foreign key exists in reference but not in target
    MissingForeignKey,
    /// Foreign key exists in target but not in reference
    ExtraForeignKey,
}

/// Schema warning (non-critical)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaWarning {
    /// Name of the database where the warning was found
    pub database: String,
    /// Category of warning
    pub warning_type: String,
    /// Human-readable description of the warning
    pub description: String,
}

/// Validation summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    /// Total number of tables checked
    pub total_tables_checked: usize,
    /// Total number of columns checked
    pub total_columns_checked: usize,
    /// Total number of indexes checked
    pub total_indexes_checked: usize,
    /// Count of critical issues found
    pub critical_issues: usize,
    /// Count of high-severity issues found
    pub high_issues: usize,
    /// Count of medium-severity issues found
    pub medium_issues: usize,
    /// Count of low-severity issues found
    pub low_issues: usize,
    /// Count of warnings found
    pub warnings: usize,
}

/// Table schema information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    /// Table name
    pub name: String,
    /// Columns in the table
    pub columns: Vec<ColumnInfo>,
    /// Indexes on the table
    pub indexes: Vec<IndexInfo>,
    /// Foreign keys defined on the table
    pub foreign_keys: Vec<ForeignKeyInfo>,
}

/// Column information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ColumnInfo {
    /// Column name
    pub name: String,
    /// SQL data type
    pub data_type: String,
    /// Whether the column has a NOT NULL constraint
    pub not_null: bool,
    /// Default value expression, if any
    pub default_value: Option<String>,
    /// Whether the column is part of the primary key
    pub primary_key: bool,
}

/// Index information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexInfo {
    /// Index name
    pub name: String,
    /// Columns included in the index
    pub columns: Vec<String>,
    /// Whether the index enforces uniqueness
    pub unique: bool,
}

/// Foreign key information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForeignKeyInfo {
    /// Local column that references the foreign table
    pub from_column: String,
    /// Target table of the foreign key
    pub to_table: String,
    /// Target column in the foreign table
    pub to_column: String,
    /// ON DELETE action, if specified
    pub on_delete: Option<String>,
}

/// Schema validator
#[derive(Debug)]
pub struct SchemaValidator {
    pub(crate) staging_connection: Option<libsql::Connection>,
    pub(crate) local_connection: Option<libsql::Connection>,
    pub(crate) cloud_connection: Option<libsql::Connection>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_info_equality() {
        let col1 = ColumnInfo {
            name: "id".to_string(),
            data_type: "INTEGER".to_string(),
            not_null: true,
            default_value: None,
            primary_key: true,
        };
        let col2 = ColumnInfo {
            name: "id".to_string(),
            data_type: "INTEGER".to_string(),
            not_null: true,
            default_value: None,
            primary_key: true,
        };
        assert_eq!(col1, col2);
    }

    #[test]
    fn test_index_info_equality() {
        let idx1 = IndexInfo {
            name: "idx_users_email".to_string(),
            columns: vec!["email".to_string()],
            unique: true,
        };
        let idx2 = IndexInfo {
            name: "idx_users_email".to_string(),
            columns: vec!["email".to_string()],
            unique: true,
        };
        assert_eq!(idx1, idx2);
    }

    #[test]
    fn test_foreign_key_info_equality() {
        let fk1 = ForeignKeyInfo {
            from_column: "user_id".to_string(),
            to_table: "users".to_string(),
            to_column: "id".to_string(),
            on_delete: Some("CASCADE".to_string()),
        };
        let fk2 = ForeignKeyInfo {
            from_column: "user_id".to_string(),
            to_table: "users".to_string(),
            to_column: "id".to_string(),
            on_delete: Some("CASCADE".to_string()),
        };
        assert_eq!(fk1, fk2);
    }

    #[test]
    fn test_issue_serder() {
        let issue = SchemaIssue {
            severity: IssueSeverity::Critical,
            database: "staging".to_string(),
            issue_type: IssueType::MissingTable,
            description: "Table 'users' is missing".to_string(),
            affected_object: "users".to_string(),
        };
        let json = serde_json::to_string(&issue).unwrap();
        assert!(json.contains("Critical"));
        let deserialized: SchemaIssue = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.severity, IssueSeverity::Critical);
    }

    #[test]
    fn test_validation_summary_counts() {
        let summary = ValidationSummary {
            total_tables_checked: 5,
            total_columns_checked: 20,
            total_indexes_checked: 3,
            critical_issues: 1,
            high_issues: 2,
            medium_issues: 3,
            low_issues: 4,
            warnings: 5,
        };
        assert_eq!(summary.critical_issues, 1);
        assert_eq!(summary.total_tables_checked, 5);
    }

    #[test]
    fn test_table_schema_construction() {
        let table = TableSchema {
            name: "users".to_string(),
            columns: vec![ColumnInfo {
                name: "id".to_string(),
                data_type: "INTEGER".to_string(),
                not_null: true,
                default_value: None,
                primary_key: true,
            }],
            indexes: vec![],
            foreign_keys: vec![],
        };
        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 1);
    }

    #[test]
    fn test_schema_validation_result_serialization() {
        let result = SchemaValidationResult {
            is_valid: true,
            databases_checked: vec!["local".to_string()],
            issues: vec![],
            warnings: vec![],
            checked_at: "2024-01-01T00:00:00Z".to_string(),
            summary: ValidationSummary {
                total_tables_checked: 0,
                total_columns_checked: 0,
                total_indexes_checked: 0,
                critical_issues: 0,
                high_issues: 0,
                medium_issues: 0,
                low_issues: 0,
                warnings: 0,
            },
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("is_valid"));
    }
}
