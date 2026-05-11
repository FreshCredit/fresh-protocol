use anyhow::Result;
use std::collections::{
    HashMap,
    HashSet,
};
use tracing::info;

use crate::types::{
    SchemaIssue,
    SchemaValidationResult,
    SchemaValidator,
    SchemaWarning,
    TableSchema,
};

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

        // Compare schemas and detect drift
        let (issues, warnings) = self.compare_schemas(&all_schemas)?;

        // Calculate summary
        let summary = self.calculate_summary(&all_schemas, &issues, &warnings);

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
        use crate::types::{
            IssueSeverity,
            IssueType,
        };

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
}
