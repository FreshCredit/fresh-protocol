//! Migration runner CLI
//!
//! Usage:
//!   `cargo run --bin run-migrations -- --local <path>`
//!   `cargo run --bin run-migrations -- --cloud <url> --token <token>`
//!   `cargo run --bin run-migrations -- --rollback <version>`

use anyhow::Result;
use freshcredit_db_migrations::MigrationRunner;
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = parse_args()?;
    let mig_dir = find_migrations_dir(args.migrations_dir)?;
    println!("📁 Migrations directory: {mig_dir:?}");

    let conn = connect_database(args.local_path, args.cloud_url, args.auth_token).await?;

    let mut runner = MigrationRunner::new(conn);
    runner.load_migrations_from_dir(&mig_dir).await?;

    let mismatches = runner.check_checksum_mismatches().await?;
    if !mismatches.is_empty() {
        eprintln!("⚠️  Checksum mismatches detected:");
        for (version, applied, current) in &mismatches {
            eprintln!(
                "   Version {}: applied={}, current={}",
                version,
                &applied[..8],
                &current[..8]
            );
        }
        if args.check_only {
            std::process::exit(1);
        }
    }

    if args.check_only {
        println!("✅ Migration check complete. No issues found.");
        return Ok(());
    }

    if let Some(version) = args.rollback_version {
        runner.rollback_migration(version).await?;
        return Ok(());
    }

    let applied = runner.run_pending_migrations().await?;
    if applied.is_empty() {
        println!("✅ No pending migrations. Database is up to date.");
    } else {
        println!("✅ Applied {} migrations: {:?}", applied.len(), applied);
    }

    Ok(())
}

struct MigrationArgs {
    local_path: Option<String>,
    cloud_url: Option<String>,
    auth_token: Option<String>,
    migrations_dir: Option<String>,
    rollback_version: Option<i64>,
    check_only: bool,
}

fn parse_args() -> Result<MigrationArgs> {
    let args: Vec<String> = env::args().collect();
    let mut result = MigrationArgs {
        local_path: None,
        cloud_url: None,
        auth_token: None,
        migrations_dir: None,
        rollback_version: None,
        check_only: false,
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--local" => {
                i += 1;
                if i < args.len() {
                    result.local_path = Some(args[i].clone());
                }
            }
            "--cloud" => {
                i += 1;
                if i < args.len() {
                    result.cloud_url = Some(args[i].clone());
                }
            }
            "--token" => {
                i += 1;
                if i < args.len() {
                    result.auth_token = Some(args[i].clone());
                }
            }
            "--migrations" => {
                i += 1;
                if i < args.len() {
                    result.migrations_dir = Some(args[i].clone());
                }
            }
            "--rollback" => {
                i += 1;
                if i < args.len() {
                    result.rollback_version = Some(args[i].parse()?);
                }
            }
            "--check" => result.check_only = true,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    Ok(result)
}

fn find_migrations_dir(migrations_dir: Option<String>) -> Result<PathBuf> {
    if let Some(dir) = migrations_dir {
        return Ok(PathBuf::from(dir));
    }

    let current = env::current_dir()?;
    if current.join("migrations").exists() {
        return Ok(current.join("migrations"));
    }

    if current
        .parent()
        .is_some_and(|p| p.join("migrations").exists())
    {
        return Ok(current
            .parent()
            .expect("parent checked above")
            .join("migrations"));
    }

    let workspace_root = current
        .ancestors()
        .find(|p| p.join("Cargo.lock").exists())
        .map(std::path::Path::to_path_buf)
        .unwrap_or(current);
    Ok(workspace_root.join("migrations"))
}

async fn connect_database(
    local_path: Option<String>,
    cloud_url: Option<String>,
    auth_token: Option<String>,
) -> Result<libsql::Connection> {
    if let Some(path) = local_path {
        println!("📦 Connecting to local database: {path}");
        let db = libsql::Builder::new_local(&path).build().await?;
        return Ok(db.connect()?);
    }

    if let Some(url) = cloud_url {
        let token = auth_token.unwrap_or_else(|| env::var("TURSO_AUTH_TOKEN").unwrap_or_default());
        println!("☁️  Connecting to cloud database: {url}");
        let db = libsql::Builder::new_remote(url, token).build().await?;
        return Ok(db.connect()?);
    }

    eprintln!("❌ No database specified. Use --local or --cloud");
    print_help();
    std::process::exit(1);
}

fn print_help() {
    println!(
        "FreshCredit Database Migration Runner

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
"
    );
}
