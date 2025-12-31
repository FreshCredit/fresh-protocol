//! Migration runner CLI
//!
//! Usage:
//!   cargo run --bin run-migrations -- --local <path>
//!   cargo run --bin run-migrations -- --cloud <url> --token <token>
//!   cargo run --bin run-migrations -- --rollback <version>

use anyhow::Result;
use freshcredit_db_migrations::MigrationRunner;
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();

    let mut local_path: Option<String> = None;
    let mut cloud_url: Option<String> = None;
    let mut auth_token: Option<String> = None;
    let mut migrations_dir: Option<String> = None;
    let mut rollback_version: Option<i64> = None;
    let mut check_only = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
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
            "--token" => {
                i += 1;
                if i < args.len() {
                    auth_token = Some(args[i].clone());
                }
            }
            "--migrations" => {
                i += 1;
                if i < args.len() {
                    migrations_dir = Some(args[i].clone());
                }
            }
            "--rollback" => {
                i += 1;
                if i < args.len() {
                    rollback_version = Some(args[i].parse()?);
                }
            }
            "--check" => check_only = true,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    // Determine migrations directory
    let mig_dir = if let Some(dir) = migrations_dir {
        PathBuf::from(dir)
    } else {
        // Default: look for migrations/ in current directory or parent
        let current = env::current_dir()?;
        if current.join("migrations").exists() {
            current.join("migrations")
        } else if current.parent().map(|p| p.join("migrations").exists()).unwrap_or(false) {
            current.parent().unwrap().join("migrations")
        } else {
            // Check from workspace root
            let workspace_root = current.ancestors()
                .find(|p| p.join("Cargo.lock").exists())
                .map(|p| p.to_path_buf())
                .unwrap_or(current);
            workspace_root.join("migrations")
        }
    };

    println!("📁 Migrations directory: {:?}", mig_dir);

    // Build database connection
    let conn = if let Some(path) = local_path {
        println!("📦 Connecting to local database: {}", path);
        let db = libsql::Builder::new_local(&path).build().await?;
        db.connect()?
    } else if let Some(url) = cloud_url {
        let token = auth_token.unwrap_or_else(|| {
            env::var("TURSO_AUTH_TOKEN").unwrap_or_default()
        });
        println!("☁️  Connecting to cloud database: {}", url);
        let db = libsql::Builder::new_remote(url, token).build().await?;
        db.connect()?
    } else {
        eprintln!("❌ No database specified. Use --local or --cloud");
        print_help();
        std::process::exit(1);
    };

    // Create runner and load migrations
    let mut runner = MigrationRunner::new(conn);
    runner.load_migrations_from_dir(&mig_dir)?;

    // Check for checksum mismatches
    let mismatches = runner.check_checksum_mismatches().await?;
    if !mismatches.is_empty() {
        eprintln!("⚠️  Checksum mismatches detected:");
        for (version, applied, current) in &mismatches {
            eprintln!("   Version {}: applied={}, current={}", version, &applied[..8], &current[..8]);
        }
        if check_only {
            std::process::exit(1);
        }
    }

    if check_only {
        println!("✅ Migration check complete. No issues found.");
        return Ok(());
    }

    // Handle rollback
    if let Some(version) = rollback_version {
        runner.rollback_migration(version).await?;
        return Ok(());
    }

    // Run pending migrations
    let applied = runner.run_pending_migrations().await?;
    if applied.is_empty() {
        println!("✅ No pending migrations. Database is up to date.");
    } else {
        println!("✅ Applied {} migrations: {:?}", applied.len(), applied);
    }

    Ok(())
}

fn print_help() {
    println!("FreshCredit Database Migration Runner

USAGE:
    run-migrations [OPTIONS]

OPTIONS:
    --local <path>       Path to local SQLite/LibSQL database
    --cloud <url>        Turso cloud database URL
    --token <token>      Auth token for cloud database (or set TURSO_AUTH_TOKEN)
    --migrations <dir>   Custom migrations directory (default: migrations/)
    --rollback <ver>     Rollback a specific migration version
    --check              Only check for pending migrations, don't apply
    --help, -h           Show this help
");
}

