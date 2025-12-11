//! Schema validation and drift detection for LibSQL databases
//!
//! This module provides tools to detect schema drift between staging, local, and cloud databases.
//! It ensures all three database types maintain the same schema structure.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::info;

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
    staging_connection: Option<libsql::Connection>,
    local_connection: Option<libsql::Connection>,
    cloud_connection: Option<libsql::Connection>,
}

impl SchemaValidator {
    /// Create a new schema validator
    pub fn new() -> Self {
        Self {
            staging_connection: None,
            local_connection: None,
            cloud_connection: None,
        }
    }

    /// Add staging database connection
    pub fn with_staging(mut self, connection: libsql::Connection) -> Self {
        self.staging_connection = Some(connection);
        self
    }

    /// Add local database connection
    pub fn with_local(mut self, connection: libsql::Connection) -> Self {
        self.local_connection = Some(connection);
        self
    }

    /// Add cloud database connection
    pub fn with_cloud(mut self, connection: libsql::Connection) -> Self {
        self.cloud_connection = Some(connection);
        self
    }

    /// Validate schema across all configured databases
    pub async fn validate(&self) -> Result<SchemaValidationResult> {
        info!("Starting schema validation across databases");

        let mut databases_checked = Vec::new();
        let mut all_schemas: HashMap<String, HashMap<String, TableSchema>> = HashMap::new();

        // Get schema from each database
        if let Some(conn) = &self.staging_connection {
            info!("Extracting schema from staging database");
            let schema = self.extract_schema(conn, "staging").await?;
            all_schemas.insert("staging".to_string(), schema);
            databases_checked.push("staging".to_string());
        }

        if let Some(conn) = &self.local_connection {
            info!("Extracting schema from local database");
            let schema = self.extract_schema(conn, "local").await?;
            all_schemas.insert("local".to_string(), schema);
            databases_checked.push("local".to_string());
        }

        if let Some(conn) = &self.cloud_connection {
            info!("Extracting schema from cloud database");
            let schema = self.extract_schema(conn, "cloud").await?;
            all_schemas.insert("cloud".to_string(), schema);
            databases_checked.push("cloud".to_string());
        }

        if all_schemas.is_empty() {
            return Err(anyhow::anyhow!("No databases configured for validation"));
        }

        // Compare schemas and detect drift
        let (issues, warnings) = self.compare_schemas(&all_schemas)?;

        // Calculate summary
        let summary = self.calculate_summary(&all_schemas, &issues, &warnings);

        let is_valid = issues
            .iter()
            .all(|i| !matches!(i.severity, IssueSeverity::Critical | IssueSeverity::High));

        Ok(SchemaValidationResult {
            is_valid,
            databases_checked,
            issues,
            warnings,
            checked_at: chrono::Utc::now().to_rfc3339(),
            summary,
        })
    }

    /// Extract schema from a database
    async fn extract_schema(
        &self,
        connection: &libsql::Connection,
        db_name: &str,
    ) -> Result<HashMap<String, TableSchema>> {
        let mut schemas = HashMap::new();

        // Get all tables
        let mut rows = connection
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
                (),
            )
            .await?;

        while let Some(row) = rows.next().await? {
            let table_name: String = row.get(0)?;

            // Get columns
            let columns = self.extract_columns(connection, &table_name).await?;

            // Get indexes
            let indexes = self.extract_indexes(connection, &table_name).await?;

            // Get foreign keys
            let foreign_keys = self.extract_foreign_keys(connection, &table_name).await?;

            schemas.insert(
                table_name.clone(),
                TableSchema {
                    name: table_name,
                    columns,
                    indexes,
                    foreign_keys,
                },
            );
        }

        info!(
            "Extracted schema for {} tables from {}",
            schemas.len(),
            db_name
        );
        Ok(schemas)
    }

    /// Extract column information for a table
    async fn extract_columns(
        &self,
        connection: &libsql::Connection,
        table_name: &str,
    ) -> Result<Vec<ColumnInfo>> {
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
    async fn extract_indexes(
        &self,
        connection: &libsql::Connection,
        table_name: &str,
    ) -> Result<Vec<IndexInfo>> {
        let mut indexes = Vec::new();
        let query = format!("PRAGMA index_list({table_name})");

        let mut rows = connection.query(&query, ()).await?;

        while let Some(row) = rows.next().await? {
            let index_name: String = row.get(1)?;
            let unique: i64 = row.get(2)?;

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
    async fn extract_foreign_keys(
        &self,
        connection: &libsql::Connection,
        table_name: &str,
    ) -> Result<Vec<ForeignKeyInfo>> {
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

    /// Compare schemas and detect drift
    fn compare_schemas(
        &self,
        all_schemas: &HashMap<String, HashMap<String, TableSchema>>,
    ) -> Result<(Vec<SchemaIssue>, Vec<SchemaWarning>)> {
        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        // Get all unique table names across all databases
        let mut all_tables: HashSet<String> = HashSet::new();
        for schemas in all_schemas.values() {
            all_tables.extend(schemas.keys().cloned());
        }

        // Compare each table across databases
        for table_name in all_tables {
            self.compare_table(table_name, all_schemas, &mut issues, &mut warnings);
        }

        Ok((issues, warnings))
    }

    /// Compare a specific table across databases
    fn compare_table(
        &self,
        table_name: String,
        all_schemas: &HashMap<String, HashMap<String, TableSchema>>,
        issues: &mut Vec<SchemaIssue>,
        warnings: &mut Vec<SchemaWarning>,
    ) {
        // Check if table exists in all databases
        for (db_name, schemas) in all_schemas {
            if !schemas.contains_key(&table_name) {
                issues.push(SchemaIssue {
                    severity: IssueSeverity::High,
                    database: db_name.clone(),
                    issue_type: IssueType::MissingTable,
                    description: format!("Table '{table_name}' is missing"),
                    affected_object: table_name.clone(),
                });
            }
        }

        // If table exists in at least one database, compare columns
        if let Some((_, reference_schema)) = all_schemas
            .iter()
            .find(|(_, s)| s.contains_key(&table_name))
        {
            if let Some(reference_table) = reference_schema.get(&table_name) {
                for (db_name, schemas) in all_schemas {
                    if let Some(table) = schemas.get(&table_name) {
                        self.compare_columns(
                            db_name,
                            &table_name,
                            &reference_table.columns,
                            &table.columns,
                            issues,
                        );
                        self.compare_indexes(
                            db_name,
                            &table_name,
                            &reference_table.indexes,
                            &table.indexes,
                            issues,
                            warnings,
                        );
                    }
                }
            }
        }
    }

    /// Compare columns between tables
    fn compare_columns(
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
    fn compare_indexes(
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
    fn calculate_summary(
        &self,
        all_schemas: &HashMap<String, HashMap<String, TableSchema>>,
        issues: &[SchemaIssue],
        warnings: &[SchemaWarning],
    ) -> ValidationSummary {
        let total_tables_checked = all_schemas.values().map(|s| s.len()).sum();
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

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}
