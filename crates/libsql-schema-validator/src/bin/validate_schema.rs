//! Schema validation CLI tool
//!
//! Usage: `cargo run --bin validate-schema -- [OPTIONS]`
//!
//! Options:
//!   `--staging <path>`    Path to staging database
//!   `--local <path>`      Path to local database
//!   `--cloud <url>`       URL to cloud database (requires `TURSO_AUTH_TOKEN` env var)
//!   --json              Output results as JSON
//!   --verbose           Show detailed output

// TAG: surface=database owner=platform-team rule=DB-001
use anyhow::Result;
use freshcredit_libsql_schema_validator::{IssueSeverity, SchemaValidator};
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = parse_cli_args();
    let mut validator = SchemaValidator::new();

    if let Some(path) = cli.staging_path {
        println!("📦 Connecting to staging database: {path}");
        let db = libsql::Builder::new_local(&path).build().await?;
        let conn = db.connect()?;
        validator = validator.with_staging(conn);
    }

    if let Some(path) = cli.local_path {
        println!("📦 Connecting to local database: {path}");
        let db = libsql::Builder::new_local(&path).build().await?;
        let conn = db.connect()?;
        validator = validator.with_local(conn);
    }

    if let Some(url) = cli.cloud_url {
        println!("📦 Connecting to cloud database: {url}");
        let auth_token = env::var("TURSO_AUTH_TOKEN").unwrap_or_else(|_| {
            eprintln!("⚠️  TURSO_AUTH_TOKEN not set, using empty token");
            String::new()
        });
        let db = libsql::Builder::new_remote(url, auth_token).build().await?;
        let conn = db.connect()?;
        validator = validator.with_cloud(conn);
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    // Run validation
    println!("\n🔍 Running schema validation...\n");
    let result = validator.validate().await?;

    // Output results
    if cli.json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_human_readable(&result, cli.verbose);
    }

    // Exit with appropriate code
    if result.is_valid {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}

struct CliArgs {
    staging_path: Option<String>,
    local_path: Option<String>,
    cloud_url: Option<String>,
    json_output: bool,
    verbose: bool,
}

fn parse_cli_args() -> CliArgs {
    let args: Vec<String> = env::args().collect();
    let mut cli = CliArgs {
        staging_path: None,
        local_path: None,
        cloud_url: None,
        json_output: false,
        verbose: false,
    };

    // TAG: surface=database owner=platform-team rule=DB-001
    let mut i = 1;
    // P10-R4: bounded — i advances through argv; exits when i reaches args.len().
    while i < args.len() {
        match args[i].as_str() {
            "--staging" => {
                i += 1;
                if i < args.len() {
                    cli.staging_path = Some(args[i].clone());
                }
            }
            "--local" => {
                i += 1;
                if i < args.len() {
                    cli.local_path = Some(args[i].clone());
                }
            }
            "--cloud" => {
                i += 1;
                if i < args.len() {
                    cli.cloud_url = Some(args[i].clone());
                }
            }
            "--json" => cli.json_output = true,
            "--verbose" => cli.verbose = true,
            _ => {}
        }
        i += 1;
    }
    cli
}

// TAG: surface=database owner=platform-team rule=DB-001
fn print_human_readable(
    result: &freshcredit_libsql_schema_validator::SchemaValidationResult,
    verbose: bool,
) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 Schema Validation Results");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    println!("🗄️  Databases Checked:");
    for db in &result.databases_checked {
        println!("   - {db}");
    }
    println!();

    println!("📈 Summary:");
    println!(
        "   Tables Checked:  {}",
        result.summary.total_tables_checked
    );
    println!(
        "   Columns Checked: {}",
        result.summary.total_columns_checked
    );
    println!(
        "   Indexes Checked: {}",
        result.summary.total_indexes_checked
    );
    println!();

    println!("🔍 Issues Found:");
    println!("   Critical: {}", result.summary.critical_issues);
    println!("   High:     {}", result.summary.high_issues);
    println!("   Medium:   {}", result.summary.medium_issues);
    println!("   Low:      {}", result.summary.low_issues);
    println!("   Warnings: {}", result.summary.warnings);
    println!();

    // TAG: surface=database owner=platform-team rule=DB-001
    if result.is_valid {
        println!("✅ Schema validation PASSED");
        println!("   All databases have consistent schemas!");
    } else {
        println!("❌ Schema validation FAILED");
        println!("   Schema drift detected between databases!");
    }
    println!();

    // Show issues
    if !result.issues.is_empty() {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🚨 Issues Details");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();

        // Group by severity
        for severity in [
            IssueSeverity::Critical,
            IssueSeverity::High,
            IssueSeverity::Medium,
            IssueSeverity::Low,
        ] {
            let issues: Vec<_> = result
                .issues
                .iter()
                .filter(|i| i.severity == severity)
                // TAG: surface=database owner=platform-team rule=DB-001
                .collect();
            if !issues.is_empty() {
                let icon = match severity {
                    IssueSeverity::Critical => "🔴",
                    IssueSeverity::High => "🟠",
                    IssueSeverity::Medium => "🟡",
                    IssueSeverity::Low => "🔵",
                };
                println!("{} {:?} Issues ({}):", icon, severity, issues.len());
                for issue in issues {
                    println!("   [{}] {}", issue.database, issue.description);
                    if verbose {
                        println!("      Type: {:?}", issue.issue_type);
                        println!("      Object: {}", issue.affected_object);
                    }
                }
                println!();
            }
        }
    }

    // Show warnings
    if !result.warnings.is_empty() && verbose {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("⚠️  Warnings");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        for warning in &result.warnings {
            println!(
                "   [{}] {}: {}",
                warning.database, warning.warning_type, warning.description
            );
        }
        println!();
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    println!("🕐 Checked at: {}", result.checked_at);
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use freshcredit_libsql_schema_validator::{
        IssueSeverity, IssueType, SchemaIssue, SchemaValidationResult, ValidationSummary,
    };

    fn make_result(
        valid: bool,
        issues: Vec<SchemaIssue>,
        warnings: Vec<freshcredit_libsql_schema_validator::SchemaWarning>,
    ) -> SchemaValidationResult {
        SchemaValidationResult {
            is_valid: valid,
            databases_checked: vec!["local".to_string()],
            summary: ValidationSummary {
                total_tables_checked: 5,
                total_columns_checked: 20,
                total_indexes_checked: 3,
                critical_issues: issues
                    .iter()
                    .filter(|i| i.severity == IssueSeverity::Critical)
                    .count(),
                high_issues: issues
                    .iter()
                    .filter(|i| i.severity == IssueSeverity::High)
                    .count(),
                medium_issues: issues
                    .iter()
                    .filter(|i| i.severity == IssueSeverity::Medium)
                    // TAG: surface=database owner=platform-team rule=DB-001
                    .count(),
                low_issues: issues
                    .iter()
                    .filter(|i| i.severity == IssueSeverity::Low)
                    .count(),
                warnings: warnings.len(),
            },
            issues,
            warnings,
            checked_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_print_human_readable_valid() {
        let result = make_result(true, vec![], vec![]);
        print_human_readable(&result, false);
    }

    #[test]
    fn test_print_human_readable_invalid_with_issues() {
        let issues = vec![
            SchemaIssue {
                database: "local".to_string(),
                severity: IssueSeverity::Critical,
                issue_type: IssueType::MissingTable,
                description: "Missing users table".to_string(),
                affected_object: "users".to_string(),
            },
            SchemaIssue {
                database: "local".to_string(),
                severity: IssueSeverity::High,
                issue_type: IssueType::MissingColumn,
                description: "Missing email column".to_string(),
                affected_object: "users.email".to_string(),
                // TAG: surface=database owner=platform-team rule=DB-001
            },
            SchemaIssue {
                database: "local".to_string(),
                severity: IssueSeverity::Medium,
                issue_type: IssueType::ColumnTypeMismatch,
                description: "Type mismatch".to_string(),
                affected_object: "users.id".to_string(),
            },
            SchemaIssue {
                database: "local".to_string(),
                severity: IssueSeverity::Low,
                issue_type: IssueType::MissingIndex,
                description: "Missing index".to_string(),
                affected_object: "users.id".to_string(),
            },
        ];
        let result = make_result(false, issues, vec![]);
        print_human_readable(&result, false);
    }

    #[test]
    fn test_print_human_readable_verbose() {
        let issues = vec![SchemaIssue {
            database: "local".to_string(),
            severity: IssueSeverity::High,
            issue_type: IssueType::MissingColumn,
            description: "Missing email".to_string(),
            affected_object: "users.email".to_string(),
        }];
        let warnings = vec![freshcredit_libsql_schema_validator::SchemaWarning {
            database: "local".to_string(),
            warning_type: "unused_index".to_string(),
            description: "Unused index".to_string(),
        }];
        let result = make_result(false, issues, warnings);
        print_human_readable(&result, true);
    }
}
