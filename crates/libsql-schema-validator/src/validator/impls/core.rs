use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tracing::info;

use crate::types::{
    SchemaIssue, SchemaValidationResult, SchemaValidator, SchemaWarning, TableSchema,
};

// TAG: surface=database owner=platform-team rule=DB-001
impl SchemaValidator {
    /// Validate schema across all configured databases
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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

        // TAG: surface=database owner=platform-team rule=DB-001
        // Compare schemas and detect drift
        let (issues, warnings) = Self::compare_schemas(&all_schemas);

        // Calculate summary
        let summary = Self::calculate_summary(&all_schemas, &issues, &warnings);

        let is_valid = issues.iter().all(|i| {
            !matches!(
                i.severity,
                crate::types::IssueSeverity::Critical | crate::types::IssueSeverity::High
            )
        });

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

        // TAG: surface=database owner=platform-team rule=DB-001
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

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Compare schemas and detect drift
    fn compare_schemas(
        all_schemas: &HashMap<String, HashMap<String, TableSchema>>,
    ) -> (Vec<SchemaIssue>, Vec<SchemaWarning>) {
        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        // Get all unique table names across all databases
        let mut all_tables: HashSet<String> = HashSet::new();
        for schemas in all_schemas.values() {
            all_tables.extend(schemas.keys().cloned());
        }

        // Compare each table across databases
        for table_name in all_tables {
            Self::compare_table(&table_name, all_schemas, &mut issues, &mut warnings);
        }

        (issues, warnings)
    }

    /// Compare a specific table across databases
    fn compare_table(
        table_name: &str,
        all_schemas: &HashMap<String, HashMap<String, TableSchema>>,
        issues: &mut Vec<SchemaIssue>,
        warnings: &mut Vec<SchemaWarning>,
    ) {
        use crate::types::{IssueSeverity, IssueType};

        // Check if table exists in all databases
        for (db_name, schemas) in all_schemas {
            // TAG: surface=database owner=platform-team rule=DB-001
            if !schemas.contains_key(table_name) {
                issues.push(SchemaIssue {
                    severity: IssueSeverity::High,
                    database: db_name.clone(),
                    issue_type: IssueType::MissingTable,
                    description: format!("Table '{table_name}' is missing"),
                    affected_object: table_name.to_string(),
                });
            }
        }

        // If table exists in at least one database, compare columns
        // Use a deterministic reference: local > staging > cloud
        let reference_schema = ["local", "staging", "cloud"]
            .iter()
            .find_map(|&name| all_schemas.get(name).filter(|s| s.contains_key(table_name)));
        if let Some(reference_schema) = reference_schema {
            if let Some(reference_table) = reference_schema.get(table_name) {
                for (db_name, schemas) in all_schemas {
                    if let Some(table) = schemas.get(table_name) {
                        Self::compare_columns(
                            db_name,
                            table_name,
                            &reference_table.columns,
                            &table.columns,
                            issues,
                        );
                        Self::compare_indexes(
                            db_name,
                            table_name,
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
}

// TAG: surface=database owner=platform-team rule=DB-001
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn test_validator() -> SchemaValidator {
        SchemaValidator {
            staging_connection: None,
            local_connection: None,
            cloud_connection: None,
        }
    }

    fn test_column(name: &str, data_type: &str) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            data_type: data_type.to_string(),
            not_null: false,
            default_value: None,
            primary_key: false,
        }
    }

    fn test_table(name: &str, columns: Vec<ColumnInfo>, indexes: Vec<IndexInfo>) -> TableSchema {
        TableSchema {
            name: name.to_string(),
            columns,
            indexes,
            foreign_keys: vec![],
        }
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    #[test]
    fn test_compare_schemas_missing_table() {
        let mut schemas: HashMap<String, HashMap<String, TableSchema>> = HashMap::new();

        let mut local = HashMap::new();
        local.insert(
            "users".to_string(),
            test_table("users", vec![test_column("id", "INTEGER")], vec![]),
        );
        schemas.insert("local".to_string(), local);

        let mut staging = HashMap::new();
        staging.insert(
            "accounts".to_string(),
            test_table("accounts", vec![test_column("id", "INTEGER")], vec![]),
        );
        schemas.insert("staging".to_string(), staging);

        let (issues, warnings) = SchemaValidator::compare_schemas(&schemas);
        assert_eq!(issues.len(), 2); // users missing in staging, accounts missing in local
        assert!(warnings.is_empty());
        assert!(issues
            .iter()
            .any(|i| i.issue_type == IssueType::MissingTable));
    }

    #[test]
    fn test_compare_schemas_equal() {
        let mut schemas: HashMap<String, HashMap<String, TableSchema>> = HashMap::new();

        let table = test_table("users", vec![test_column("id", "INTEGER")], vec![]);
        let mut local = HashMap::new();
        local.insert("users".to_string(), table.clone());
        schemas.insert("local".to_string(), local);

        // TAG: surface=database owner=platform-team rule=DB-001
        let mut staging = HashMap::new();
        staging.insert("users".to_string(), table);
        schemas.insert("staging".to_string(), staging);

        let (issues, warnings) = SchemaValidator::compare_schemas(&schemas);
        assert!(issues.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_compare_schemas_empty() {
        let schemas: HashMap<String, HashMap<String, TableSchema>> = HashMap::new();
        let (issues, warnings) = SchemaValidator::compare_schemas(&schemas);
        assert!(issues.is_empty());
        assert!(warnings.is_empty());
    }

    #[tokio::test]
    async fn test_extract_schema_from_db() {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .expect("Failed to build in-memory database");
        let connection = db.connect().expect("Failed to connect");

        connection
            .execute(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL)",
                (),
            )
            .await
            .unwrap();

        // TAG: surface=database owner=platform-team rule=DB-001
        connection
            .execute("CREATE INDEX idx_email ON users(email)", ())
            .await
            .unwrap();

        let validator = test_validator();
        let schema = validator.extract_schema(&connection, "test").await.unwrap();

        assert_eq!(schema.len(), 1);
        assert!(schema.contains_key("users"));
        let table = schema.get("users").unwrap();
        assert_eq!(table.columns.len(), 2);
        assert_eq!(table.indexes.len(), 1);
    }

    #[tokio::test]
    async fn test_validate_with_single_db() {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .expect("Failed to build in-memory database");
        let connection = db.connect().expect("Failed to connect");

        connection
            .execute("CREATE TABLE users (id INTEGER PRIMARY KEY)", ())
            .await
            .unwrap();

        let validator = SchemaValidator::new().with_local(connection);
        let result = validator.validate().await.unwrap();

        assert!(result.is_valid);
        assert_eq!(result.databases_checked, vec!["local"]);
        assert!(result.issues.is_empty());
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    #[tokio::test]
    async fn test_validate_detects_drift() {
        let local_db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .expect("Failed to build local database");
        let local_conn = local_db.connect().expect("Failed to connect");
        local_conn
            .execute(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT)",
                (),
            )
            .await
            .unwrap();

        let staging_db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .expect("Failed to build staging database");
        let staging_conn = staging_db.connect().expect("Failed to connect");
        staging_conn
            .execute("CREATE TABLE users (id INTEGER PRIMARY KEY)", ())
            .await
            .unwrap();

        let validator = SchemaValidator::new()
            .with_local(local_conn)
            .with_staging(staging_conn);

        let result = validator.validate().await.unwrap();
        assert!(!result.is_valid);
        assert!(result
            .issues
            .iter()
            .any(|i| i.issue_type == IssueType::MissingColumn));
    }
}
