use serde::{
    Deserialize,
    Serialize,
};

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IssueSeverity {
    Critical, // Schema drift that breaks functionality
    High,     // Missing tables or columns
    Medium,   // Missing indexes
    Low,      // Minor differences
}

/// Issue type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
    pub default_value: Option<String>,
    pub primary_key: bool,
}

/// Index information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

/// Foreign key information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
