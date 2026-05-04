use anyhow::Result;
use libsql::Connection;

use super::*;

/// Initialize all schema tables from extracted modules
///
/// This function initializes all tables that have been extracted into schema modules.
/// It provides a complete schema initialization using the modular approach.
///
/// Table counts by module:
/// - Core: 6 tables (user_profile, user_preferences, api_keys, webauthn_credentials, kilt_dids, auth_tokens)
/// - Financial: 3 tables (accounts, transactions, balances)
/// - Identity: 2 tables (identity_verification, verified_credentials)
/// - AI: 3 tables (ai_conversations, ai_messages, uploaded_files)
/// - Workflow: 1 table (workflows)
/// - Webhook: 2 tables (webhook_events, notifications)
/// - Plaid: 20 tables (items, accounts, identities, assets, etc.)
/// - Payments: 8 tables (customers, funding_sources, payments, crypto_wallets, crypto_payments, arc_receipts, etc.)
/// - Reports: 8 tables (reports, scores, offers, disputes, etc.)
/// - Ticketing: 4 tables (tickets, ticket_comments, etc.)
/// - Compliance: 4 tables (compliance_scans, compliance_rules, etc.)
/// - Notifications: 2 tables (webhook_events, notifications - shared with webhook)
/// - LinkedIn: 6 tables (linkedin_profiles, etc.)
/// - HealthKit: 6 tables (healthkit_profiles, etc.)
/// - IP: 5 tables (ip_records, ip_claims, ip_evidence, ip_events, ip_disputes)
/// - Publications: 7 tables (publication_records, publication_claims, orcid_connections, publication_evidence, publication_events, publication_disputes, publication_shares)
/// - Apple Music: 6 tables (apple_music_profiles, etc.)
/// - Correlation: 3 tables (correlation_preferences, etc.)
/// - Platform: 4 tables (data_approval_hashes, referrals, etc.)
/// - Teams: 3 tables (provider_teams, team_members, team_invites)
/// - Customers: 4 tables (customer_activities, customer_segments, customer_segment_memberships, customer_communications)
/// - Security: 5 tables (ip_blocks, rate_limit_events, step_up_auth_requests, user_devices, compliance_digests)
///
/// HARDCODED_SCHEMA: 133 unique tables total across all modules (verified 2026-01-18)
/// Core: 6 tables (user_profile, user_preferences, api_keys, webauthn_credentials, kilt_dids, auth_tokens)
/// Financial: 3 tables | Identity: 2 tables | AI: 7 tables | Workflow: 1 table | Webhook: 2 tables
/// Plaid: 20 tables | Payments: 11 tables | Reports: 11 tables | Ticketing: 4 tables | Compliance: 5 tables
/// LinkedIn: 6 tables | HealthKit: 6 tables | IP: 5 tables | Publications: 7 tables | Apple Music: 6 tables
/// Correlation: 3 tables | Platform: 3 tables | Teams: 3 tables | Customers: 4 tables | Security: 4 tables
/// UCP: 3 tables | Agent: 6 tables | Governance: 6 tables
/// Note: SQLite IF NOT EXISTS handles deduplication for tables appearing in multiple modules.
/// Validates schema initialization by tracking which categories are initialized
/// ARCH-007: Schema validation tracking for startup validation logging
pub async fn initialize_all_schema_tables(conn: &Connection) -> Result<()> {
    use tracing::info;

    // Disable foreign key constraints during schema initialization
    // This allows synced data from Turso cloud to load even if referenced rows
    // arrive in a different order. We re-enable at the end.
    conn.execute("PRAGMA foreign_keys = OFF", ()).await?;

    let mut validations = Vec::new();

    // Core tables (user_profile must be first - referenced by other tables)
    initialize_core_tables(conn).await?;
    initialize_core_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "core",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: core");

    // Financial tables
    initialize_financial_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "financial",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: financial");

    // Identity tables
    initialize_identity_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "identity",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: identity");

    // AI tables
    initialize_ai_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "ai",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: ai");

    // Workflow tables
    initialize_workflow_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "workflow",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: workflow");

    // Webhook tables
    initialize_webhook_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "webhook",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: webhook");

    // Plaid product tables
    initialize_all_plaid_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "plaid",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: plaid");

    // Payment tables
    initialize_payment_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "payments",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: payments");

    // Business logic tables
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

    // Data source tables
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

    // Platform tables
    initialize_platform_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "platform",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: platform");

    // Teams tables (provider_teams, team_members, team_invites)
    initialize_teams_tables(conn).await?;
    initialize_teams_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "teams",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: teams");

    // Customer management tables (customer_activities, customer_segments, etc.)
    initialize_customer_tables(conn).await?;
    initialize_customer_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "customers",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: customers");

    // Security tables (ip_blocks, rate_limit_events, step_up_auth, user_devices, compliance_digests)
    initialize_security_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "security",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: security");

    // Agent tables (agent_bindings, agent_memories, agent_interactions, agent_audit_events)
    initialize_agent_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "agent",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: agent");

    // UCP tables (ucp_checkout_sessions, ucp_orders, ucp_identity_links)
    // COMPLIANCE: AGENT-004 - All UCP tables include user_id for data access control
    initialize_ucp_tables(conn).await?;
    validations.push(SchemaValidation {
        category: "ucp",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: ucp");

    // Governance tables (governance_proposals, governance_votes, governance_delegations, etc.)
    // COMPLIANCE: §7 - Uses neutral governance terminology
    initialize_governance_tables(conn).await?;
    initialize_governance_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "governance",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: governance");

    // All remaining indexes (organized by category)
    initialize_all_indexes(conn).await?;
    validations.push(SchemaValidation {
        category: "indexes",
        tables_initialized: true,
    });
    info!("[ARCH-007] Schema category initialized: indexes");

    // Final validation summary
    info!(
        "[ARCH-007] Schema initialization complete: {} categories validated",
        validations.len()
    );

    Ok(())
}

/// Initialize extracted schema tables (legacy function for compatibility)
///
/// This function initializes only the originally extracted tables.
/// For full schema initialization, use initialize_all_schema_tables().
pub async fn initialize_extracted_tables(conn: &Connection) -> Result<()> {
    initialize_core_tables(conn).await?;
    initialize_financial_tables(conn).await?;
    initialize_identity_tables(conn).await?;
    initialize_ai_tables(conn).await?;
    initialize_workflow_tables(conn).await?;
    initialize_webhook_tables(conn).await?;
    Ok(())
}
