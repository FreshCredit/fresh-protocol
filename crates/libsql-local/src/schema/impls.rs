use anyhow::Result;
use libsql::Connection;

use super::*;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize all schema tables from extracted modules
///
/// ARCH-007: Schema validation tracking for startup validation logging
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_all_schema_tables(conn: &Connection) -> Result<()> {
    use tracing::info;

    // Disable foreign key constraints during schema initialization
    conn.execute("PRAGMA foreign_keys = OFF", ()).await?;

    let mut validations = Vec::new();

    init_core_group(conn, &mut validations).await?;
    init_business_group(conn, &mut validations).await?;
    init_data_source_group(conn, &mut validations).await?;
    init_platform_group(conn, &mut validations).await?;

    info!(
        "[ARCH-007] Schema initialization complete: {} categories validated",
        validations.len()
    );

    Ok(())
}

/// Initialize core and foundational schema modules.
async fn init_core_group(conn: &Connection, validations: &mut Vec<SchemaValidation>) -> Result<()> {
    use tracing::info;

    initialize_core_tables(conn).await?;
    initialize_core_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "core",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: core");

    // TAG: surface=database owner=platform-team rule=DB-001
    initialize_financial_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "financial",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: financial");

    initialize_identity_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "identity",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: identity");

    initialize_ai_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "ai",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: ai");

    initialize_workflow_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "workflow",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: workflow");

    initialize_webhook_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "webhook",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: webhook");

    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize business logic schema modules.
async fn init_business_group(
    conn: &Connection,
    validations: &mut Vec<SchemaValidation>,
) -> Result<()> {
    use tracing::info;

    initialize_all_plaid_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "plaid",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: plaid");

    initialize_payment_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "payments",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: payments");

    initialize_all_reports_tables_with_seed(conn).await?;
    validations.push(SchemaValidation {
        category: "reports",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: reports");

    initialize_ticketing_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "ticketing",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: ticketing");

    initialize_compliance_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "compliance",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: compliance");

    // TAG: surface=database owner=platform-team rule=DB-001
    initialize_notification_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "notifications",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: notifications");

    Ok(())
}

/// Initialize data source schema modules.
async fn init_data_source_group(
    conn: &Connection,
    validations: &mut Vec<SchemaValidation>,
) -> Result<()> {
    use tracing::info;

    initialize_linkedin_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "linkedin",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: linkedin");

    initialize_healthkit_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "healthkit",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: healthkit");

    initialize_ip_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "ip",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: ip");

    // TAG: surface=database owner=platform-team rule=DB-001
    initialize_publication_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "publications",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: publications");

    initialize_apple_music_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "apple_music",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: apple_music");

    initialize_correlation_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "correlation",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: correlation");

    initialize_platform_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "platform",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: platform");

    Ok(())
}

/// Initialize platform and operational schema modules.
/// # Errors
///
/// Returns an error if the operation fails.
async fn init_platform_group(
    conn: &Connection,
    validations: &mut Vec<SchemaValidation>,
) -> Result<()> {
    use tracing::info;

    // TAG: surface=database owner=platform-team rule=DB-001
    initialize_teams_tables(conn).await?;
    initialize_teams_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "teams",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: teams");

    initialize_customer_tables(conn).await?;
    initialize_customer_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "customers",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: customers");

    initialize_security_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "security",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: security");

    initialize_agent_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "agent",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: agent");

    initialize_ucp_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "ucp",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: ucp");

    // TAG: surface=database owner=platform-team rule=DB-001
    initialize_governance_tables(conn).await?;
    initialize_governance_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "governance",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: governance");

    initialize_all_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "indexes",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: indexes");

    Ok(())
}

/// Initialize all schema tables without demo seeding.
/// Use this for per-user cloud databases where the browser will populate data.
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_all_schema_tables_no_seed(conn: &Connection) -> Result<()> {
    use tracing::info;

    // Disable foreign key constraints during schema initialization
    conn.execute("PRAGMA foreign_keys = OFF", ()).await?;

    let mut validations = Vec::new();

    init_core_group(conn, &mut validations).await?;
    // Business group uses DDL-only reports initialization
    init_business_group_no_seed(conn, &mut validations).await?;
    init_data_source_group(conn, &mut validations).await?;
    init_platform_group(conn, &mut validations).await?;

    info!(
        "[ARCH-007] Schema initialization complete (no seed): {} categories validated",
        validations.len()
    );

    Ok(())
}

async fn init_business_group_no_seed(
    conn: &Connection,
    validations: &mut Vec<SchemaValidation>,
) -> Result<()> {
    use tracing::info;

    initialize_all_plaid_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "plaid",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: plaid");

    initialize_payment_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "payments",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: payments");

    initialize_all_reports_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "reports",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: reports");

    initialize_ticketing_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "ticketing",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: ticketing");

    initialize_compliance_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "compliance",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: compliance");

    initialize_notification_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "notifications",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: notifications");

    Ok(())
}

/// Initialize extracted schema tables (legacy function for compatibility)
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_extracted_tables(conn: &Connection) -> Result<()> {
    initialize_core_tables(conn).await?;
    initialize_financial_tables(conn).await?;
    initialize_identity_tables(conn).await?;
    initialize_ai_tables(conn).await?;
    initialize_workflow_tables(conn).await?;
    initialize_webhook_tables(conn).await?;
    Ok(())
}
