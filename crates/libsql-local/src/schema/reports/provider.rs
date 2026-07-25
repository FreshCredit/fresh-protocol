use anyhow::Result;
use libsql::Connection;

// TAG: surface=database owner=platform-team rule=DB-001
/// Initialize provider offers and dispute tables
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn initialize_provider_tables(conn: &Connection) -> Result<()> {
    // COMPLIANCE: §2 - Neutral matching only, no recommendations
    conn.execute(
        "CREATE TABLE IF NOT EXISTS provider_offers (
            id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            product_type TEXT NOT NULL,
            loan_amount_min_cents INTEGER,
            loan_amount_max_cents INTEGER,
            apr_min_percent REAL,
            apr_max_percent REAL,
            term_options_months TEXT,
            fees_json TEXT,
            rewards_json TEXT,
            included_states TEXT,
            excluded_states TEXT,
            included_zip_codes TEXT,
            nationwide BOOLEAN DEFAULT TRUE,
            customer_segment TEXT DEFAULT 'all',
            customer_profiles TEXT,
            blockscore_model_id TEXT,
            ab_test_variant TEXT,
            ab_test_allocation INTEGER,
            is_active BOOLEAN DEFAULT TRUE,
            version INTEGER DEFAULT 1,
            blockchain_hash TEXT,
            block_number INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (provider_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    // DB ownership: local per-user DB report-data disputes; the shared DB
    // owns the payment-dispute table of the same name
    // (schema_manager/tables/impls/payments.rs).
    conn.execute(
        "CREATE TABLE IF NOT EXISTS disputes (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            dispute_type TEXT NOT NULL,
            disputed_item_type TEXT,
            disputed_item_id TEXT,
            dispute_reason TEXT NOT NULL,
            dispute_description TEXT,
            supporting_documents TEXT,
            dispute_status TEXT DEFAULT 'pending',
            resolution TEXT,
            resolved_at DATETIME,
            raw_dispute_data TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES user_profile (id) ON DELETE CASCADE
        )",
        (),
    )
    .await?;

    Ok(())
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Seed demo provider offers if table is empty
/// COMPLIANCE: §4 - Template rules - these are example offers for demonstration
/// NOTE: These are demo offers. Providers must define their own terms in production.
#[allow(clippy::too_many_lines)]
pub(crate) async fn seed_demo_provider_offers(conn: &Connection) -> Result<()> {
    // Check if offers already exist
    let mut rows = conn
        .query("SELECT COUNT(*) as count FROM provider_offers", ())
        .await?;
    if let Some(row) = rows.next().await? {
        let count: i64 = row.get(0)?;
        if count > 0 {
            return Ok(()); // Already seeded
        }
    }

    // First, create demo provider user profiles (required for foreign key constraint)
    // These are system demo providers, not real users
    let demo_providers = [
        (
            "demo_provider_001",
            "FlexiLoan Financial",
            "flexiloan@demo.freshcredit.com",
        ),
        (
            "demo_provider_002",
            "Rewards Plus Bank",
            "rewardsplus@demo.freshcredit.com",
        ),
        (
            "demo_provider_003",
            "AutoDrive Finance",
            "autodrive@demo.freshcredit.com",
        ),
        (
            // TAG: surface=database owner=platform-team rule=DB-001
            "demo_provider_004",
            "HomeFirst Lending",
            "homefirst@demo.freshcredit.com",
        ),
        (
            "demo_provider_005",
            "EduFund Services",
            "edufund@demo.freshcredit.com",
        ),
        (
            "demo_provider_006",
            "BizFlex Capital",
            "bizflex@demo.freshcredit.com",
        ),
    ];

    for (id, name, email) in demo_providers {
        // Use INSERT OR IGNORE to avoid duplicates
        let _ = conn.execute(
            "INSERT OR IGNORE INTO user_profile (id, platform_user_id, azure_id, email, display_name, role, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, 'provider', datetime('now'), datetime('now'))",
            libsql::params![id, id, id, email, name],
        ).await;
    }

    // Seed demo offers
    conn.execute(
        "INSERT INTO provider_offers (id, provider_id, name, description, product_type,
            loan_amount_min_cents, loan_amount_max_cents, apr_min_percent, apr_max_percent,
            term_options_months, nationwide, is_active, created_at, updated_at)
         VALUES ('demo_offer_personal_001', 'demo_provider_001', 'FlexiLoan Personal',
            'Flexible personal loan with competitive rates', 'PersonalLoan',
            500000, 5000000, 7.99, 24.99, '[12, 24, 36, 48, 60]', 1, 1, datetime('now'), datetime('now'))",
        (),
    ).await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    conn.execute(
        "INSERT INTO provider_offers (id, provider_id, name, description, product_type,
            loan_amount_min_cents, loan_amount_max_cents, apr_min_percent, apr_max_percent,
            term_options_months, nationwide, is_active, created_at, updated_at)
         VALUES ('demo_offer_credit_001', 'demo_provider_002', 'Rewards Plus Card',
            'Earn 2% cashback on all purchases with no annual fee', 'CreditCard',
            50000, 2500000, 15.99, 26.99, '[]', 1, 1, datetime('now'), datetime('now'))",
        (),
    )
    .await?;

    conn.execute(
        "INSERT INTO provider_offers (id, provider_id, name, description, product_type,
            loan_amount_min_cents, loan_amount_max_cents, apr_min_percent, apr_max_percent,
            term_options_months, nationwide, is_active, created_at, updated_at)
         VALUES ('demo_offer_auto_001', 'demo_provider_003', 'AutoDrive Financing',
            'New and used vehicle financing with flexible terms', 'AutoLoan',
            1000000, 7500000, 4.99, 18.99, '[36, 48, 60, 72, 84]', 1, 1, datetime('now'), datetime('now'))",
        (),
    ).await?;

    conn.execute(
        "INSERT INTO provider_offers (id, provider_id, name, description, product_type,
            loan_amount_min_cents, loan_amount_max_cents, apr_min_percent, apr_max_percent,
            term_options_months, included_states, nationwide, is_active, created_at, updated_at)
         VALUES ('demo_offer_mortgage_001', 'demo_provider_004', 'HomeFirst Mortgage',
            'Conventional and FHA mortgage options for home buyers', 'Mortgage',
            10000000, 100000000, 5.25, 7.50, '[180, 240, 360]',
            '[\"CA\", \"TX\", \"NY\", \"FL\", \"WA\"]', 0, 1, datetime('now'), datetime('now'))",
        (),
    )
    .await?;

    // TAG: surface=database owner=platform-team rule=DB-001
    conn.execute(
        "INSERT INTO provider_offers (id, provider_id, name, description, product_type,
            loan_amount_min_cents, loan_amount_max_cents, apr_min_percent, apr_max_percent,
            term_options_months, nationwide, is_active, created_at, updated_at)
         VALUES ('demo_offer_student_001', 'demo_provider_005', 'EduFund Student Loan',
            'Private student loans for undergraduate and graduate students', 'StudentLoan',
            100000, 15000000, 4.49, 14.99, '[60, 84, 120, 180]', 1, 1, datetime('now'), datetime('now'))",
        (),
    ).await?;

    conn.execute(
        "INSERT INTO provider_offers (id, provider_id, name, description, product_type,
            loan_amount_min_cents, loan_amount_max_cents, apr_min_percent, apr_max_percent,
            term_options_months, customer_segment, nationwide, is_active, created_at, updated_at)
         VALUES ('demo_offer_business_001', 'demo_provider_006', 'BizFlex Credit Line',
            'Revolving business credit line for small business owners', 'LineOfCredit',
            1000000, 25000000, 8.99, 22.99, '[12, 24]', 'business', 1, 1, datetime('now'), datetime('now'))",
        (),
    ).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_reports_module_compiles() {
        let _ = 1 + 1; // Compile-time verification
    }
}
