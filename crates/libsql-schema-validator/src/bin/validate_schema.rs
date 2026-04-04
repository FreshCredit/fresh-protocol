//! Schema validation CLI tool
//!
//! Usage: cargo run --bin validate-schema -- [OPTIONS]
//!
//! Options:
//!   --staging <path>    Path to staging database
//!   --local <path>      Path to local database
//!   --cloud <url>       URL to cloud database (requires TURSO_AUTH_TOKEN env var)
//!   --json              Output results as JSON
//!   --verbose           Show detailed output

use anyhow::Result;
use freshcredit_libsql_schema_validator::{IssueSeverity, SchemaValidator};
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();

    let mut staging_path: Option<String> = None;
    let mut local_path: Option<String> = None;
    let mut cloud_url: Option<String> = None;
    let mut json_output = false;
    let mut verbose = false;

    // Parse arguments
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--staging" => {
                i += 1;
                if i < args.len() {
                    staging_path = Some(args[i].clone());
                }
            }
            "--local" => {
                i += 1;
                if i < args.len() {
                    local_path = Some(args[i].clone());
                }
            }
            "--cloud" => {
                i += 1;
                if i < args.len() {
                    cloud_url = Some(args[i].clone());
                }
            }
            "--json" => json_output = true,
            "--verbose" => verbose = true,
            _ => {}
        }
        i += 1;
    }

    // Build validator
    let mut validator = SchemaValidator::new();

    if let Some(path) = staging_path {
        println!("📦 Connecting to staging database: {path}");
        let db = libsql::Builder::new_local(&path).build().await?;
        let conn = db.connect()?;
        validator = validator.with_staging(conn);
    }

    if let Some(path) = local_path {
        println!("📦 Connecting to local database: {path}");
        let db = libsql::Builder::new_local(&path).build().await?;
        let conn = db.connect()?;
        validator = validator.with_local(conn);
    }

    if let Some(url) = cloud_url {
        println!("📦 Connecting to cloud database: {url}");
        let auth_token = env::var("TURSO_AUTH_TOKEN").unwrap_or_else(|_| {
            eprintln!("⚠️  TURSO_AUTH_TOKEN not set, using empty token");
            String::new()
        });
        let db = libsql::Builder::new_remote(url, auth_token).build().await?;
        let conn = db.connect()?;
        validator = validator.with_cloud(conn);
    }

    // Run validation
    println!("\n🔍 Running schema validation...\n");
    let result = validator.validate().await?;

    // Output results
    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_human_readable(&result, verbose);
    }

    // Exit with appropriate code
    if result.is_valid {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}

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

    println!("🕐 Checked at: {}", result.checked_at);
    println!();
}
