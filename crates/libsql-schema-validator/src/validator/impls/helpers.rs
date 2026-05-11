use anyhow::Result;
use freshcredit_libsql_common::security::validate_identifier;
use std::collections::HashSet;

use crate::types::{
    ColumnInfo,
    ForeignKeyInfo,
    IndexInfo,
    IssueSeverity,
    IssueType,
    SchemaIssue,
    SchemaValidator,
    SchemaWarning,
    ValidationSummary,
};

impl SchemaValidator {
    /// Extract column information for a table
    /// P0-SECURITY: Added identifier validation to prevent SQL injection
    pub(crate) async fn extract_columns(
        &self,
        connection: &libsql::Connection,
        table_name: &str,
    ) -> Result<Vec<ColumnInfo>> {
        // P0-SECURITY: Validate table name to prevent SQL injection
        validate_identifier(table_name)?;

        let mut columns = Vec::new();
        let query = format!("PRAGMA table_info({table_name})");

        let mut rows = connection.query(&query, ()).await?;

        while let Some(row) = rows.next().await? {
            let name: String = row.get(1)?;
            let data_type: String = row.get(2)?;
            let not_null: i64 = row.get(3)?;
            let default_value: Option<String> = row.get(4).ok();
            let primary_key: i64 = row.get(5)?;

            columns.push(ColumnInfo {
                name,
                data_type,
                not_null: not_null != 0,
                default_value,
                primary_key: primary_key != 0,
            });
        }

        Ok(columns)
    }

    /// Extract index information for a table
    /// P0-SECURITY: Added identifier validation to prevent SQL injection
    pub(crate) async fn extract_indexes(
        &self,
        connection: &libsql::Connection,
        table_name: &str,
    ) -> Result<Vec<IndexInfo>> {
        // P0-SECURITY: Validate table name to prevent SQL injection
        validate_identifier(table_name)?;

        let mut indexes = Vec::new();
        let query = format!("PRAGMA index_list({table_name})");

        let mut rows = connection.query(&query, ()).await?;

        while let Some(row) = rows.next().await? {
            let index_name: String = row.get(1)?;
            let unique: i64 = row.get(2)?;

            // P0-SECURITY: Validate index name to prevent SQL injection
            validate_identifier(&index_name)?;

            // Get columns for this index
            let columns_query = format!("PRAGMA index_info({index_name})");
            let mut col_rows = connection.query(&columns_query, ()).await?;
            let mut columns = Vec::new();

            while let Some(col_row) = col_rows.next().await? {
                let col_name: String = col_row.get(2)?;
                columns.push(col_name);
            }

            indexes.push(IndexInfo {
                name: index_name,
                columns,
                unique: unique != 0,
            });
        }

        Ok(indexes)
    }

    /// Extract foreign key information for a table
    /// P0-SECURITY: Added identifier validation to prevent SQL injection
    pub(crate) async fn extract_foreign_keys(
        &self,
        connection: &libsql::Connection,
        table_name: &str,
    ) -> Result<Vec<ForeignKeyInfo>> {
        // P0-SECURITY: Validate table name to prevent SQL injection
        validate_identifier(table_name)?;

        let mut foreign_keys = Vec::new();
        let query = format!("PRAGMA foreign_key_list({table_name})");

        let mut rows = connection.query(&query, ()).await?;

        while let Some(row) = rows.next().await? {
            let to_table: String = row.get(2)?;
            let from_column: String = row.get(3)?;
            let to_column: String = row.get(4)?;
            let on_delete: Option<String> = row.get(6).ok();

            foreign_keys.push(ForeignKeyInfo {
                from_column,
                to_table,
                to_column,
                on_delete,
            });
        }

        Ok(foreign_keys)
    }

    /// Compare columns between tables
    pub(crate) fn compare_columns(
        &self,
        db_name: &str,
        table_name: &str,
        reference_columns: &[ColumnInfo],
        actual_columns: &[ColumnInfo],
        issues: &mut Vec<SchemaIssue>,
    ) {
        let ref_col_names: HashSet<_> = reference_columns.iter().map(|c| &c.name).collect();
        let actual_col_names: HashSet<_> = actual_columns.iter().map(|c| &c.name).collect();

        // Check for missing columns
        for col in reference_columns {
            if !actual_col_names.contains(&col.name) {
                issues.push(SchemaIssue {
                    severity: IssueSeverity::High,
                    database: db_name.to_string(),
                    issue_type: IssueType::MissingColumn,
                    description: format!(
                        "Column '{}' is missing from table '{}'",
                        col.name, table_name
                    ),
                    affected_object: format!("{}.{}", table_name, col.name),
                });
            }
        }

        // Check for extra columns
        for col in actual_columns {
            if !ref_col_names.contains(&col.name) {
                issues.push(SchemaIssue {
                    severity: IssueSeverity::Low,
                    database: db_name.to_string(),
                    issue_type: IssueType::ExtraColumn,
                    description: format!(
                        "Extra column '{}' found in table '{}'",
                        col.name, table_name
                    ),
                    affected_object: format!("{}.{}", table_name, col.name),
                });
            }
        }
    }

    /// Compare indexes between tables
    ///
    /// Note: `_warnings` parameter kept for API consistency with other compare methods
    #[allow(clippy::ptr_arg)]
    pub(crate) fn compare_indexes(
        &self,
        db_name: &str,
        table_name: &str,
        reference_indexes: &[IndexInfo],
        actual_indexes: &[IndexInfo],
        issues: &mut Vec<SchemaIssue>,
        _warnings: &mut Vec<SchemaWarning>,
    ) {
        let _ref_idx_names: HashSet<_> = reference_indexes.iter().map(|i| &i.name).collect();
        let actual_idx_names: HashSet<_> = actual_indexes.iter().map(|i| &i.name).collect();

        // Check for missing indexes
        for idx in reference_indexes {
            if !actual_idx_names.contains(&idx.name) {
                issues.push(SchemaIssue {
                    severity: IssueSeverity::Medium,
                    database: db_name.to_string(),
                    issue_type: IssueType::MissingIndex,
                    description: format!(
                        "Index '{}' is missing from table '{}'",
                        idx.name, table_name
                    ),
                    affected_object: format!("{}.{}", table_name, idx.name),
                });
            }
        }
    }

    /// Calculate validation summary
    pub(crate) fn calculate_summary(
        &self,
        all_schemas: &std::collections::HashMap<
            String,
            std::collections::HashMap<String, crate::types::TableSchema>,
        >,
        issues: &[SchemaIssue],
        warnings: &[SchemaWarning],
    ) -> ValidationSummary {
        let total_tables_checked = all_schemas
            .values()
            .map(std::collections::HashMap::len)
            .sum();
        let total_columns_checked = all_schemas
            .values()
            .flat_map(|s| s.values())
            .map(|t| t.columns.len())
            .sum();
        let total_indexes_checked = all_schemas
            .values()
            .flat_map(|s| s.values())
            .map(|t| t.indexes.len())
            .sum();

        let critical_issues = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Critical)
            .count();
        let high_issues = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::High)
            .count();
        let medium_issues = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Medium)
            .count();
        let low_issues = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Low)
            .count();

        ValidationSummary {
            total_tables_checked,
            total_columns_checked,
            total_indexes_checked,
            critical_issues,
            high_issues,
            medium_issues,
            low_issues,
            warnings: warnings.len(),
        }
    }
}
