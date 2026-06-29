//! Migration runner CLI
//!
//! Usage:
//!   `cargo run --bin run-migrations -- --local <path>`
//!   `cargo run --bin run-migrations -- --cloud <url> --token <token>`
//!   `cargo run --bin run-migrations -- --rollback <version>`

// TAG: surface=database owner=platform-team rule=DB-001
use anyhow::Result;
use freshcredit_db_migrations::MigrationRunner;
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Install rustls ring crypto provider before any libsql/TLS connection is made.
    let _ = rustls::crypto::ring::default_provider().install_default();
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

    // TAG: surface=database owner=platform-team rule=DB-001
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
    parse_args_from(&env::args().collect::<Vec<String>>())
}

fn parse_args_from(args: &[String]) -> Result<MigrationArgs> {
    let mut result = MigrationArgs {
        local_path: None,
        cloud_url: None,
        auth_token: None,
        migrations_dir: None,
        rollback_version: None,
        check_only: false,
    };

    // TAG: surface=database owner=platform-team rule=DB-001
    let mut i = 1;
    while i < args.len() {
        match args.get(i).map(std::string::String::as_str) {
            Some("--local") => {
                i += 1;
                if let Some(val) = args.get(i) {
                    result.local_path = Some(val.clone());
                }
            }
            Some("--cloud") => {
                i += 1;
                if let Some(val) = args.get(i) {
                    result.cloud_url = Some(val.clone());
                }
            }
            Some("--token") => {
                i += 1;
                if let Some(val) = args.get(i) {
                    result.auth_token = Some(val.clone());
                }
            }
            Some("--migrations") => {
                i += 1;
                if let Some(val) = args.get(i) {
                    result.migrations_dir = Some(val.clone());
                }
            }
            Some("--rollback") => {
                i += 1;
                // TAG: surface=database owner=platform-team rule=DB-001
                if let Some(val) = args.get(i) {
                    result.rollback_version = Some(val.parse()?);
                }
            }
            Some("--check") => result.check_only = true,
            Some("--help" | "-h") => {
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

    // TAG: surface=database owner=platform-team rule=DB-001
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

    // TAG: surface=database owner=data-team rule=DB-001
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_migrations_dir_with_override() {
        let result = find_migrations_dir(Some("/tmp/migrations".to_string())).unwrap();
        assert_eq!(result, PathBuf::from("/tmp/migrations"));
    }

    #[test]
    fn test_find_migrations_dir_fallback() {
        let result = find_migrations_dir(None).unwrap();
        assert!(result.ends_with("migrations"));
    }

    #[test]
    fn test_print_help_does_not_panic() {
        print_help();
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    #[test]
    fn test_parse_args_local() {
        let args = vec![
            "run-migrations".to_string(),
            "--local".to_string(),
            "/tmp/db.db".to_string(),
        ];
        let result = parse_args_from(&args).unwrap();
        assert_eq!(result.local_path, Some("/tmp/db.db".to_string()));
    }

    #[test]
    fn test_parse_args_cloud() {
        let args = vec![
            "run-migrations".to_string(),
            "--cloud".to_string(),
            "https://db.turso.io".to_string(),
            "--token".to_string(),
            "abc".to_string(),
        ];
        let result = parse_args_from(&args).unwrap();
        assert_eq!(result.cloud_url, Some("https://db.turso.io".to_string()));
        assert_eq!(result.auth_token, Some("abc".to_string()));
    }

    #[test]
    fn test_parse_args_migrations_dir() {
        let args = vec![
            "run-migrations".to_string(),
            "--migrations".to_string(),
            "/tmp/mig".to_string(),
        ];
        let result = parse_args_from(&args).unwrap();
        assert_eq!(result.migrations_dir, Some("/tmp/mig".to_string()));
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    #[test]
    fn test_parse_args_rollback() {
        let args = vec![
            "run-migrations".to_string(),
            "--rollback".to_string(),
            "42".to_string(),
        ];
        let result = parse_args_from(&args).unwrap();
        assert_eq!(result.rollback_version, Some(42));
    }

    #[test]
    fn test_parse_args_check_only() {
        let args = vec!["run-migrations".to_string(), "--check".to_string()];
        let result = parse_args_from(&args).unwrap();
        assert!(result.check_only);
    }

    #[test]
    fn test_parse_args_empty() {
        let args = vec!["run-migrations".to_string()];
        let result = parse_args_from(&args).unwrap();
        assert!(result.local_path.is_none());
        assert!(result.cloud_url.is_none());
        assert!(!result.check_only);
    }

    #[test]
    fn test_parse_args_invalid_rollback() {
        let args = vec![
            "run-migrations".to_string(),
            "--rollback".to_string(),
            "not-a-number".to_string(),
        ];
        assert!(parse_args_from(&args).is_err());
    }
}
