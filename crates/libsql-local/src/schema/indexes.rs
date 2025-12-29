//! Database indexes schema definitions
//!
//! Contains all index creation for performance optimization.
//! Indexes are grouped by table category.
//!
//! COMPLIANCE: §10 Unified Database Schema Architecture

use anyhow::Result;
use libsql::Connection;

/// Initialize Plaid table indexes
pub async fn initialize_plaid_indexes(conn: &Connection) -> Result<()> {
    conn.execute("CREATE INDEX IF NOT EXISTS idx_assets_user_id ON assets(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_balances_account_id ON balances(account_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_consumer_reports_user_id ON consumer_reports(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_employment_user_id ON employment(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_enrich_transaction_id ON enrich(transaction_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_income_user_id ON income(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_income_verification_user_id ON income_verification(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_identity_verification_user_id ON identity_verification(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_investments_holdings_account_id ON investments_holdings(account_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_investments_securities_ticker ON investments_securities(ticker_symbol)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_investments_transactions_account_id ON investments_transactions(account_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_layer_account_id ON layer(account_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_liabilities_account_id ON liabilities(account_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_monitor_item_id ON monitor(item_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_recurring_transactions_account_id ON recurring_transactions(account_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_signal_evaluations_account_id ON signal_evaluations(account_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_statements_account_id ON statements(account_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_transactions_sync_item_id ON transactions_sync(item_id)", ()).await?;
    Ok(())
}

/// Initialize payment table indexes
pub async fn initialize_payment_indexes(conn: &Connection) -> Result<()> {
    conn.execute("CREATE INDEX IF NOT EXISTS idx_customers_user_id ON customers(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_funding_sources_customer_id ON funding_sources(customer_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_payments_customer_id ON payments(customer_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_stripe_plaid_payments_user_id ON stripe_plaid_payments(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_virtual_accounts_customer_id ON virtual_accounts(customer_id)", ()).await?;
    Ok(())
}

/// Initialize business logic table indexes
pub async fn initialize_business_indexes(conn: &Connection) -> Result<()> {
    conn.execute("CREATE INDEX IF NOT EXISTS idx_reports_user_id ON reports(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_scores_user_id ON scores(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_scores_provider_id ON scores(provider_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_offers_user_id ON offers(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_offers_provider_id ON offers(provider_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_disputes_user_id ON disputes(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_verification_requests_user_id ON verification_requests(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_phone_verification_codes_phone ON phone_verification_codes(phone_number)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_phone_verification_codes_expires ON phone_verification_codes(expires_at)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_verified_credentials_user_id ON verified_credentials(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_verified_credentials_did_uri ON verified_credentials(did_uri)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_identities_account_id ON identities(account_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_identities_user_id ON identities(user_id)", ()).await?;
    Ok(())
}

/// Initialize notification and webhook indexes
pub async fn initialize_notification_indexes(conn: &Connection) -> Result<()> {
    conn.execute("CREATE INDEX IF NOT EXISTS idx_webhook_events_user_id ON webhook_events(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_webhook_events_status ON webhook_events(status)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_webhook_events_provider ON webhook_events(provider)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_notifications_user_id ON notifications(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_notifications_read_status ON notifications(read_status)", ()).await?;
    Ok(())
}

/// Initialize ticketing system indexes
pub async fn initialize_ticketing_indexes(conn: &Connection) -> Result<()> {
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tickets_user_id ON tickets(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tickets_status ON tickets(status)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tickets_priority ON tickets(priority)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tickets_type ON tickets(ticket_type)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tickets_created_at ON tickets(created_at)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_ticket_comments_ticket_id ON ticket_comments(ticket_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_ticket_comments_author_id ON ticket_comments(author_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_ticket_assignments_ticket_id ON ticket_assignments(ticket_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_ticket_assignments_assignee_id ON ticket_assignments(assignee_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_ticket_sla_events_ticket_id ON ticket_sla_events(ticket_id)", ()).await?;
    Ok(())
}

/// Initialize compliance monitoring indexes
pub async fn initialize_compliance_indexes(conn: &Connection) -> Result<()> {
    conn.execute("CREATE INDEX IF NOT EXISTS idx_compliance_scans_status ON compliance_scans(status)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_compliance_scans_framework ON compliance_scans(framework)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_compliance_scans_started_at ON compliance_scans(started_at)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_compliance_rules_framework ON compliance_rules(framework)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_compliance_rules_severity ON compliance_rules(severity)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_compliance_rules_is_active ON compliance_rules(is_active)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_compliance_findings_scan_id ON compliance_findings(scan_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_compliance_findings_rule_id ON compliance_findings(rule_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_compliance_findings_severity ON compliance_findings(severity)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_compliance_findings_status ON compliance_findings(status)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_compliance_evidence_finding_id ON compliance_evidence(finding_id)", ()).await?;
    Ok(())
}

/// Initialize platform table indexes
pub async fn initialize_platform_indexes(conn: &Connection) -> Result<()> {
    conn.execute("CREATE INDEX IF NOT EXISTS idx_data_approval_hashes_user_id ON data_approval_hashes(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_data_approval_hashes_blockchain_hash ON data_approval_hashes(blockchain_hash)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_data_approval_hashes_created_at ON data_approval_hashes(created_at)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_referrals_referrer_user_id ON referrals(referrer_user_id)", ()).await?;
    conn.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_referrals_code ON referrals(referral_code)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_referrals_status ON referrals(status)", ()).await?;
    conn.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_platform_metrics_date ON platform_metrics(metric_date)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_sales_pipeline_user_id ON sales_pipeline(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_sales_pipeline_stage ON sales_pipeline(stage)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_sales_pipeline_hubspot_deal_id ON sales_pipeline(hubspot_deal_id)", ()).await?;
    Ok(())
}

/// Initialize LinkedIn table indexes
pub async fn initialize_linkedin_indexes(conn: &Connection) -> Result<()> {
    conn.execute("CREATE INDEX IF NOT EXISTS idx_linkedin_profiles_user_id ON linkedin_profiles(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_linkedin_experiences_profile ON linkedin_experiences(linkedin_profile_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_linkedin_education_profile ON linkedin_education(linkedin_profile_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_linkedin_skills_profile ON linkedin_skills(linkedin_profile_id)", ()).await?;
    Ok(())
}

/// Initialize HealthKit table indexes
pub async fn initialize_healthkit_indexes(conn: &Connection) -> Result<()> {
    conn.execute("CREATE INDEX IF NOT EXISTS idx_healthkit_profiles_user_id ON healthkit_profiles(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_healthkit_records_profile ON healthkit_records(healthkit_profile_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_healthkit_records_type ON healthkit_records(record_type)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_healthkit_records_date ON healthkit_records(start_date)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_healthkit_workouts_profile ON healthkit_workouts(healthkit_profile_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_healthkit_activity_profile ON healthkit_activity_summaries(healthkit_profile_id)", ()).await?;
    Ok(())
}

/// Initialize correlation table indexes
pub async fn initialize_correlation_indexes(conn: &Connection) -> Result<()> {
    conn.execute("CREATE INDEX IF NOT EXISTS idx_correlation_prefs_user ON correlation_preferences(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_correlation_insights_user ON correlation_insights(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_correlation_insights_type ON correlation_insights(insight_type)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_correlation_metrics_user ON correlation_metrics(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_correlation_metrics_type ON correlation_metrics(metric_type)", ()).await?;
    Ok(())
}

/// Initialize Apple Music table indexes
pub async fn initialize_apple_music_indexes(conn: &Connection) -> Result<()> {
    conn.execute("CREATE INDEX IF NOT EXISTS idx_apple_music_profiles_user ON apple_music_profiles(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_apple_music_songs_user ON apple_music_library_songs(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_apple_music_albums_user ON apple_music_library_albums(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_apple_music_playlists_user ON apple_music_playlists(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_apple_music_recently_played_user ON apple_music_recently_played(user_id)", ()).await?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_apple_music_genre_stats_user ON apple_music_genre_stats(user_id)", ()).await?;
    Ok(())
}

/// Initialize all indexes
pub async fn initialize_all_indexes(conn: &Connection) -> Result<()> {
    initialize_plaid_indexes(conn).await?;
    initialize_payment_indexes(conn).await?;
    initialize_business_indexes(conn).await?;
    initialize_notification_indexes(conn).await?;
    initialize_ticketing_indexes(conn).await?;
    initialize_compliance_indexes(conn).await?;
    initialize_platform_indexes(conn).await?;
    initialize_linkedin_indexes(conn).await?;
    initialize_healthkit_indexes(conn).await?;
    initialize_correlation_indexes(conn).await?;
    initialize_apple_music_indexes(conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_indexes_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}

