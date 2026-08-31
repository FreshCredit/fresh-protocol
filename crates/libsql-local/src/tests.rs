use super::*;

/// Test that the schema contains exactly 156 tables as documented
/// `HARDCODED_SCHEMA`: 150 unique tables in modular schema (verified 2026-08-25)
/// Added: `provider_teams`, `team_members`, `team_invites` (Teams)
/// Added: `customer_activities`, `customer_segments`, `customer_segment_memberships`, `customer_communications` (Customers)
/// Added: `offer_analytics`, `offer_ab_test_results`, `offer_events` (Offer Analytics)
/// Added: `bridge_transfers`, `gateway_sessions`, `gateway_transactions` (Circle Arc Phase 2)
/// Added: `vault_category_vectors` (Vault vectorization, plus internal vector-index tables)
#[tokio::test]
async fn test_schema_table_count() {
    let client = LocalClient::new_in_memory().await.unwrap();
    client.initialize_schema().await.unwrap();

    // TAG: surface=database owner=platform-team rule=DB-001
    // Query sqlite_master for table count
    let mut rows = client
        .query(
            "SELECT COUNT(*) as count FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            vec![],
        )
        .await
        .unwrap();

    let row = rows.next().await.unwrap().unwrap();
    let count: i64 = row.get(0).unwrap();
    // HARDCODED_SCHEMA: 136 tables (verified 2026-01-21)
    // Core: 6 tables (user_profile, user_preferences, api_keys, webauthn_credentials, kilt_dids, auth_tokens)
    // Financial: 3 tables (accounts, transactions, balances)
    // Identity: 2 tables (identity_verification, verified_credentials)
    // AI: 7 tables (ai_conversations, ai_messages, uploaded_files, ai_feedback, ai_model_configs, ai_request_logs, ai_usage_metrics)
    // Workflow: 1 table (workflows)
    // Webhook: 2 tables (webhook_events, notifications)
    // Plaid: 20 tables (items, identities, assets, income, income_verification, employment, layer, enrich, monitor, signal_evaluations, consumer_reports, phone_verification_codes, liabilities, statements, investments_holdings, investments_securities, investments_transactions, recurring_transactions, transactions_sync, verification_requests)
    // Payments: 11 tables (customers, funding_sources, payments, crypto_wallets, crypto_payments, arc_receipts, bridge_transfers, gateway_sessions, gateway_transactions, virtual_accounts, stripe_plaid_payments)
    // Reports: 11 tables (reports, scores, offers, provider_offers, user_offer_engagements, disputes, offer_analytics, offer_ab_test_results, offer_events, audit_events, data_approval_hashes)
    // Ticketing: 4 tables (tickets, ticket_comments, ticket_assignments, ticket_sla_events)
    // Compliance: 5 tables (compliance_scans, compliance_rules, compliance_findings, compliance_evidence, compliance_digests)
    // LinkedIn: 6 tables (linkedin_profiles, linkedin_experiences, linkedin_education, linkedin_skills, linkedin_certifications, linkedin_languages)
    // HealthKit: 6 tables (healthkit_profiles, healthkit_records, healthkit_workouts, healthkit_activity_summaries, healthkit_clinical_records, healthkit_correlations)
    // IP: 5 tables (ip_records, ip_claims, ip_evidence, ip_events, ip_disputes)
    // Note: 2 additional tables from tech debt remediation (ARCH-P2-001 profile schema alignment)
    // Publications: 7 tables (publication_records, publication_claims, orcid_connections, publication_evidence, publication_events, publication_disputes, publication_shares)
    // Apple Music: 6 tables (apple_music_profiles, apple_music_library_songs, apple_music_library_albums, apple_music_playlists, apple_music_recently_played, apple_music_genre_stats)
    // Correlation: 3 tables (correlation_preferences, correlation_insights, correlation_metrics)
    // Platform: 3 tables (referrals, platform_metrics, sales_pipeline)
    // Teams: 3 tables (provider_teams, team_members, team_invites)
    // Customers: 4 tables (customer_activities, customer_segments, customer_segment_memberships, customer_communications)
    // Security: 5 tables (sessions, user_devices, ip_blocks, rate_limit_events, step_up_auth_requests)
    // UCP: 3 tables (ucp_checkout_sessions, ucp_orders, ucp_identity_links)
    // Agent: 6 tables (agent_bindings, agent_memories, agent_interactions, agent_audit_events, agentfs_kv_store, agentfs_tool_calls)
    // Governance: 6 tables (governance_proposals, governance_votes, governance_delegations, governance_treasury, governance_treasury_transactions, governance_stewards)
    // +3 tables from 2026-07 migrations (share canonical hash, share turso db, user-identities primary)
    // +1 table for multi-institution Plaid items (plaid_items)
    // +1 table for browser-first payment methods (payment_methods)
    // +1 table for deletion propagation tombstone ledger (sync_deletions, slice C1)
    // +8 GTT trust-score tables (gtt_physio_stream, gtt_fin_stream,
    // gtt_ling_stream, gtt_composite_index, gtt_model_metadata,
    // gtt_user_meta, gtt_user_baseline, gtt_labels)
    // +1 consumer approved_data mirror table (vault-as-source-of-truth, phase 2)
    // +1 vault_category_vectors table (vault vectorization; internal vector-index tables add 2 more)
    // +1 native_health_staging table (Apple Health / Google Health Connect staging)
    assert_eq!(count, 157, "Schema should contain exactly 157 tables");
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Test that critical tables exist in the schema
#[tokio::test]
async fn test_critical_tables_exist() {
    let client = LocalClient::new_in_memory().await.unwrap();
    client.initialize_schema().await.unwrap();

    let critical_tables = vec![
        "user_profile",
        "accounts",
        "transactions",
        "balances",
        "reports",
        "models",
        "identity_verification",
        "workflows",
        "scores",
        "ai_conversations",
        "ai_messages",
        "data_approval_hashes",
        "referrals",
        "platform_metrics",
        "sales_pipeline",
        // LinkedIn tables (L1: Multi-Source Integration)
        "linkedin_profiles",
        "linkedin_experiences",
        "linkedin_education",
        "linkedin_skills",
        "linkedin_certifications",
        "linkedin_languages",
        // HealthKit tables (H1: Multi-Source Integration)
        "healthkit_profiles",
        // TAG: surface=database owner=platform-team rule=DB-001
        "healthkit_records",
        "healthkit_workouts",
        "healthkit_activity_summaries",
        "healthkit_clinical_records",
        "healthkit_correlations",
        // Apple Music tables (AM1: Multi-Source Integration)
        "apple_music_profiles",
        "apple_music_library_songs",
        "apple_music_library_albums",
        "apple_music_playlists",
        "apple_music_recently_played",
        "apple_music_genre_stats",
        // Deletion propagation tombstone ledger (slice C1)
        "sync_deletions",
        // Consumer approved-data mirror (vault-as-source-of-truth, phase 2)
        "approved_data",
    ];

    for table in critical_tables {
        let mut rows = client
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name=?",
                vec![libsql::Value::Text(table.to_string())],
            )
            .await
            .unwrap();

        let row = rows.next().await.unwrap();
        assert!(
            row.is_some(),
            "Critical table '{table}' should exist in schema"
        );
    }
}

/// Test that user profile CRUD operations work
#[tokio::test]
async fn test_user_profile_crud() {
    let client = LocalClient::new_in_memory().await.unwrap();
    client.initialize_schema().await.unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    let profile = UserProfile {
        // TAG: surface=database owner=platform-team rule=DB-001
        id: "test-user-123".to_string(),
        platform_user_id: "platform-123".to_string(),
        azure_id: "azure-123".to_string(),
        email: "test@example.com".to_string(),
        display_name: "Test User".to_string(),
        given_name: Some("Test".to_string()),
        family_name: Some("User".to_string()),
        surname: None,
        mobile_phone: None,
        job_title: None,
        street_address: None,
        city: None,
        state_province: None,
        postal_code: None,
        country_region: None,
        date_of_birth: None,
        ssn_last_four: None,
        employment_status: None,
        annual_income: None,
        // ARCH-P2-001: Extended profile fields
        phone_number: None,
        preferred_name: None,
        emergency_contact_name: None,
        emergency_contact_phone: None,
        employer_name: None,
        role: "consumer".to_string(),
        is_admin: false,
        provider_onboarding_complete: false,
        mfa_enabled: false,
        mfa_verified_at: None,
        tenant_id: "tenant-123".to_string(),
        object_id: "object-123".to_string(),
        verified_id_credential_id: None,
        verified_id_status: "pending".to_string(),
        verified_id_issued_at: None,
        consumer_verified_id_credential_id: None,
        consumer_verified_id_status: "pending".to_string(),
        consumer_verified_id_issued_at: None,
        provider_verified_id_credential_id: None,
        provider_verified_id_status: "pending".to_string(),
        provider_verified_id_issued_at: None,
        created_at: now.clone(),
        updated_at: now,
    };

    // Store user profile
    client.store_user_profile(&profile).await.unwrap();

    // TAG: surface=database owner=platform-team rule=DB-001
    // Verify user exists
    let retrieved = client.get_user_profile("platform-123").await.unwrap();
    assert!(
        retrieved.is_some(),
        "User profile should exist after creation"
    );

    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id, "test-user-123");
    assert_eq!(retrieved.email, "test@example.com");
    assert_eq!(retrieved.role, "consumer");
}

/// Test query execution returns rows
#[tokio::test]
async fn test_query_execution() {
    let client = LocalClient::new_in_memory().await.unwrap();
    client.initialize_schema().await.unwrap();

    let mut rows = client.query("SELECT 1 as value", vec![]).await.unwrap();

    let row = rows.next().await.unwrap().unwrap();
    let value: i64 = row.get(0).unwrap();
    assert_eq!(value, 1);
}

/// Test execute returns affected rows
#[tokio::test]
async fn test_execute_returns_rows_affected() {
    let client = LocalClient::new_in_memory().await.unwrap();
    client.initialize_schema().await.unwrap();

    // Create a simple test table without foreign key constraints
    client
        .execute(
            "CREATE TABLE IF NOT EXISTS test_execute_table (id TEXT PRIMARY KEY, value TEXT)",
            vec![],
        )
        .await
        .unwrap();

    // TAG: surface=database owner=platform-team rule=DB-001
    // Insert a test row
    let result = client
        .execute(
            "INSERT INTO test_execute_table (id, value) VALUES ('test-id', 'test-value')",
            vec![],
        )
        .await
        .unwrap();

    assert_eq!(result, 1); // 1 row affected
}

/// Test schema idempotency - running initialize twice should not error
#[tokio::test]
async fn test_schema_initialization_idempotent() {
    let client = LocalClient::new_in_memory().await.unwrap();

    // Initialize twice
    client.initialize_schema().await.unwrap();
    client.initialize_schema().await.unwrap();

    // Verify schema still valid
    let mut rows = client
        .query(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
            vec![],
        )
        .await
        .unwrap();

    let row = rows.next().await.unwrap().unwrap();
    let count: i64 = row.get(0).unwrap();
    assert!(count > 0);
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Test user profile update
#[tokio::test]
async fn test_user_profile_update() {
    let client = LocalClient::new_in_memory().await.unwrap();
    client.initialize_schema().await.unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    let mut profile = UserProfile {
        id: "update-test-user".to_string(),
        platform_user_id: "platform-update-123".to_string(),
        azure_id: "azure-update-123".to_string(),
        email: "update@example.com".to_string(),
        display_name: "Original Name".to_string(),
        given_name: None,
        family_name: None,
        surname: None,
        mobile_phone: None,
        job_title: None,
        street_address: None,
        city: None,
        state_province: None,
        postal_code: None,
        country_region: None,
        date_of_birth: None,
        ssn_last_four: None,
        employment_status: None,
        annual_income: None,
        // ARCH-P2-001: Extended profile fields
        phone_number: None,
        preferred_name: None,
        emergency_contact_name: None,
        emergency_contact_phone: None,
        employer_name: None,
        role: "consumer".to_string(),
        is_admin: false,
        provider_onboarding_complete: false,
        mfa_enabled: false,
        mfa_verified_at: None,
        tenant_id: "tenant".to_string(),
        object_id: "object".to_string(),
        verified_id_credential_id: None,
        verified_id_status: "pending".to_string(),
        verified_id_issued_at: None,
        consumer_verified_id_credential_id: None,
        consumer_verified_id_status: "pending".to_string(),
        consumer_verified_id_issued_at: None,
        provider_verified_id_credential_id: None,
        provider_verified_id_status: "pending".to_string(),
        provider_verified_id_issued_at: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    // TAG: surface=database owner=platform-team rule=DB-001
    // Store initial profile
    client.store_user_profile(&profile).await.unwrap();

    // Update and store again
    profile.display_name = "Updated Name".to_string();
    profile.updated_at = chrono::Utc::now().to_rfc3339();
    client.store_user_profile(&profile).await.unwrap();

    // Verify update
    let retrieved = client
        .get_user_profile("platform-update-123")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retrieved.display_name, "Updated Name");
}

/// Test user profile not found
#[tokio::test]
async fn test_user_profile_not_found() {
    let client = LocalClient::new_in_memory().await.unwrap();
    client.initialize_schema().await.unwrap();

    let result = client.get_user_profile("nonexistent-user").await.unwrap();
    assert!(result.is_none());
}
