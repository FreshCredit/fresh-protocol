//! Schema synchronization tests
//!
//! These tests verify that Rust schema definitions, Turso cloud schema,
//! and SQL migration files are all in sync to prevent deployment failures.
//!
//! The embedded replica pattern requires schema alignment BEFORE deployment:
//! 1. Turso cloud tables are synced to local DB on container startup
//! 2. Rust code then creates indexes on those tables
//! 3. If Turso tables have different columns than Rust expects, index creation fails

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// Represents a column extracted from schema definitions
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaColumn {
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
}

/// Represents a table schema
#[derive(Debug, Clone)]
pub struct TableDef {
    pub name: String,
    pub columns: Vec<SchemaColumn>,
}

/// Extract table definitions from SQL CREATE TABLE statements
pub fn parse_sql_schema(sql: &str) -> HashMap<String, TableDef> {
    let mut tables = HashMap::new();

    // Find all CREATE TABLE statements
    let create_table_re =
        regex::Regex::new(r"(?is)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)\s*\(([^;]+)\)")
            .unwrap();

    for cap in create_table_re.captures_iter(sql) {
        let table_name = cap.get(1).unwrap().as_str().to_lowercase();
        let columns_str = cap.get(2).unwrap().as_str();

        let columns = parse_columns(columns_str);

        tables.insert(
            table_name.clone(),
            TableDef {
                name: table_name,
                columns,
            },
        );
    }

    tables
}

/// Parse column definitions from CREATE TABLE body
fn parse_columns(columns_str: &str) -> Vec<SchemaColumn> {
    let mut columns = Vec::new();

    // Split by comma, but be careful of commas in FOREIGN KEY clauses
    let mut in_parens = 0;
    let mut current_col = String::new();
    let mut col_strs = Vec::new();

    for ch in columns_str.chars() {
        match ch {
            '(' => {
                in_parens += 1;
                current_col.push(ch);
            }
            ')' => {
                in_parens -= 1;
                current_col.push(ch);
            }
            ',' if in_parens == 0 => {
                col_strs.push(current_col.trim().to_string());
                current_col = String::new();
            }
            _ => current_col.push(ch),
        }
    }
    if !current_col.trim().is_empty() {
        col_strs.push(current_col.trim().to_string());
    }

    for col_str in col_strs {
        let col_str = col_str.trim();

        // Skip FOREIGN KEY, PRIMARY KEY, UNIQUE, CHECK constraints
        let upper = col_str.to_uppercase();
        if upper.starts_with("FOREIGN KEY")
            || upper.starts_with("PRIMARY KEY")
            || upper.starts_with("UNIQUE")
            || upper.starts_with("CHECK")
            || upper.starts_with("CONSTRAINT")
        {
            continue;
        }

        // Parse column: name type [NOT NULL] [DEFAULT ...]
        let parts: Vec<&str> = col_str.split_whitespace().collect();
        if parts.len() >= 2 {
            let name = parts[0].to_lowercase();
            let data_type = parts[1].to_uppercase();
            let not_null = col_str.to_uppercase().contains("NOT NULL");

            columns.push(SchemaColumn {
                name,
                data_type,
                not_null,
            });
        }
    }

    columns
}

/// Load and parse the unified_schema.sql migration file
pub fn load_migration_schema(project_root: &Path) -> Result<HashMap<String, TableDef>> {
    let migration_path = project_root.join("migrations/unified_schema.sql");
    let sql = fs::read_to_string(&migration_path)?;
    Ok(parse_sql_schema(&sql))
}

/// Compare two schemas and return differences
pub fn compare_schemas(
    source_name: &str,
    source: &HashMap<String, TableDef>,
    target_name: &str,
    target: &HashMap<String, TableDef>,
) -> Vec<String> {
    let mut differences = Vec::new();

    // Check for tables in source but not in target
    for table_name in source.keys() {
        if !target.contains_key(table_name) {
            differences.push(format!(
                "Table '{table_name}' exists in {source_name} but not in {target_name}"
            ));
        }
    }

    // Check for column differences in common tables
    for (table_name, source_table) in source {
        if let Some(target_table) = target.get(table_name) {
            let source_cols: HashSet<_> = source_table.columns.iter().map(|c| &c.name).collect();
            let target_cols: HashSet<_> = target_table.columns.iter().map(|c| &c.name).collect();

            // Columns in source but not target
            for col in source_cols.difference(&target_cols) {
                differences.push(format!(
                    "Column '{table_name}.{col}' exists in {source_name} but not in {target_name}"
                ));
            }
        }
    }

    differences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_create_table() {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT NOT NULL,
                name TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
        "#;

        let tables = parse_sql_schema(sql);
        assert_eq!(tables.len(), 1);

        let users = tables.get("users").unwrap();
        assert_eq!(users.columns.len(), 4);
        assert!(users.columns.iter().any(|c| c.name == "id"));
        assert!(users
            .columns
            .iter()
            .any(|c| c.name == "email" && c.not_null));
    }

    #[test]
    fn test_parse_table_with_foreign_key() {
        let sql = r#"
            CREATE TABLE accounts (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                account_id TEXT NOT NULL,
                balance REAL DEFAULT 0.0,
                FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
            );
        "#;

        let tables = parse_sql_schema(sql);
        let accounts = tables.get("accounts").unwrap();

        // Should have 4 columns, not including the FOREIGN KEY line
        assert_eq!(accounts.columns.len(), 4);
        assert!(accounts.columns.iter().any(|c| c.name == "user_id"));
        assert!(accounts.columns.iter().any(|c| c.name == "account_id"));
    }

    #[test]
    fn test_compare_schemas_finds_missing_table() {
        let mut source = HashMap::new();
        source.insert(
            "users".to_string(),
            TableDef {
                name: "users".to_string(),
                columns: vec![],
            },
        );
        source.insert(
            "accounts".to_string(),
            TableDef {
                name: "accounts".to_string(),
                columns: vec![],
            },
        );

        let mut target = HashMap::new();
        target.insert(
            "users".to_string(),
            TableDef {
                name: "users".to_string(),
                columns: vec![],
            },
        );

        let diffs = compare_schemas("source", &source, "target", &target);
        assert!(diffs
            .iter()
            .any(|d| d.contains("accounts") && d.contains("not in target")));
    }

    #[test]
    fn test_compare_schemas_finds_missing_column() {
        let mut source = HashMap::new();
        source.insert(
            "users".to_string(),
            TableDef {
                name: "users".to_string(),
                columns: vec![
                    SchemaColumn {
                        name: "id".to_string(),
                        data_type: "TEXT".to_string(),
                        not_null: false,
                    },
                    SchemaColumn {
                        name: "email".to_string(),
                        data_type: "TEXT".to_string(),
                        not_null: true,
                    },
                ],
            },
        );

        let mut target = HashMap::new();
        target.insert(
            "users".to_string(),
            TableDef {
                name: "users".to_string(),
                columns: vec![SchemaColumn {
                    name: "id".to_string(),
                    data_type: "TEXT".to_string(),
                    not_null: false,
                }],
            },
        );

        let diffs = compare_schemas("source", &source, "target", &target);
        assert!(diffs
            .iter()
            .any(|d| d.contains("email") && d.contains("not in target")));
    }

    #[test]
    fn test_load_migration_schema() {
        // This test requires the project root path
        let project_root = std::env::current_dir()
            .unwrap()
            .ancestors()
            .find(|p| p.join("migrations/unified_schema.sql").exists())
            .map(|p| p.to_path_buf());

        if let Some(root) = project_root {
            let result = load_migration_schema(&root);
            assert!(
                result.is_ok(),
                "Failed to load migration schema: {:?}",
                result.err()
            );

            let tables = result.unwrap();
            // Should have many tables
            assert!(
                tables.len() > 50,
                "Expected 50+ tables, got {}",
                tables.len()
            );

            // Check for key tables
            assert!(
                tables.contains_key("user_profile"),
                "Missing user_profile table"
            );
            assert!(tables.contains_key("accounts"), "Missing accounts table");
            assert!(tables.contains_key("workflows"), "Missing workflows table");
        }
    }
}
