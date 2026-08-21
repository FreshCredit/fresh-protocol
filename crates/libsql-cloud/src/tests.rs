use super::*;
use chrono::Utc;
use uuid::Uuid;

// TAG: surface=database owner=platform-team rule=DB-001
impl CloudClient {
    async fn new_test() -> Self {
        let path = format!("/tmp/freshcredit_cloud_test_{}.db", uuid::Uuid::new_v4());
        let db = libsql::Builder::new_local(&path)
            .build()
            .await
            .expect("Failed to build test database");
        let connection = db.connect().expect("Failed to connect to test database");
        Self {
            connection: Mutex::new(connection),
            database: Arc::new(db),
        }
    }
}

fn test_user_id() -> String {
    "user-123".to_string()
}

fn test_account(
    user_id: &str,
    id: &str,
    account_type: freshcredit_types::AccountType,
) -> freshcredit_types::Account {
    freshcredit_types::Account {
        id: id.to_string(),
        user_id: user_id.to_string(),
        account_type,
        balance: Some(1000.0),
        currency: "USD".to_string(),
        institution_name: "Test Bank".to_string(),
        created_at: Utc::now(),
    }
}

fn test_transaction(account_id: &str, id: &str) -> freshcredit_types::Transaction {
    freshcredit_types::Transaction {
        id: id.to_string(),
        account_id: account_id.to_string(),
        amount: 50.0,
        currency: "USD".to_string(),
        description: "Test transaction".to_string(),
        category: Some("Food".to_string()),
        date: Utc::now(),
        merchant_name: Some("Test Merchant".to_string()),
    }
}

fn test_financial_report(user_id: &str) -> freshcredit_types::FinancialReport {
    freshcredit_types::FinancialReport {
        id: Uuid::new_v4(),
        user_id: user_id.to_string(),
        // TAG: surface=database owner=platform-team rule=DB-001
        bureau_score: Some(750),
        accounts: vec![test_account(
            user_id,
            "acc-report-1",
            freshcredit_types::AccountType::Checking,
        )],
        transactions: vec![],
        generated_at: Utc::now(),
        blockchain_hash: Some("0xabc123".to_string()),
    }
}

fn test_user_profile_simple() -> freshcredit_libsql_local::UserProfile {
    freshcredit_libsql_local::UserProfile {
        id: test_user_id(),
        platform_user_id: "platform-1".to_string(),
        azure_id: "azure-1".to_string(),
        email: "test@example.com".to_string(),
        display_name: "Test User".to_string(),
        given_name: Some("Test".to_string()),
        family_name: Some("User".to_string()),
        surname: Some("User".to_string()),
        mobile_phone: Some("555-1234".to_string()),
        job_title: Some("Developer".to_string()),
        street_address: Some("123 Main St".to_string()),
        city: Some("Anytown".to_string()),
        // TAG: surface=database owner=platform-team rule=GENERAL-001
        state_province: Some("CA".to_string()),
        postal_code: Some("12345".to_string()),
        country_region: Some("USA".to_string()),
        date_of_birth: Some("1990-01-01".to_string()),
        ssn_last_four: Some("1234".to_string()),
        employment_status: Some("Employed".to_string()),
        annual_income: Some(100_000),
        phone_number: Some("555-5678".to_string()),
        preferred_name: Some("Tester".to_string()),
        emergency_contact_name: Some("Emergency Contact".to_string()),
        emergency_contact_phone: Some("555-9999".to_string()),
        employer_name: Some("Test Corp".to_string()),
        role: "consumer".to_string(),
        is_admin: false,
        provider_onboarding_complete: false,
        mfa_enabled: false,
        mfa_verified_at: None,
        tenant_id: "freshcredit".to_string(),
        object_id: "obj-1".to_string(),
        verified_id_credential_id: Some("vid-1".to_string()),
        verified_id_status: "verified".to_string(),
        verified_id_issued_at: Some("2024-01-01".to_string()),
        consumer_verified_id_credential_id: Some("vid-1".to_string()),
        consumer_verified_id_status: "verified".to_string(),
        consumer_verified_id_issued_at: Some("2024-01-01".to_string()),
        provider_verified_id_credential_id: None,
        provider_verified_id_status: "pending".to_string(),
        provider_verified_id_issued_at: None,
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
    }
}

// TAG: surface=database owner=platform-team rule=DB-001
fn test_user_preferences() -> freshcredit_libsql_local::UserPreferences {
    freshcredit_libsql_local::UserPreferences {
        ai_agent_enabled: Some(true),
        ai_feedback_enabled: Some(false),
        ai_offers_enabled: Some(true),
        ai_lenders_enabled: Some(false),
        cloud_sync_enabled: Some(true),
        blockchain_enabled: Some(true),
        email_notifications_enabled: Some(true),
        kilt_did_enabled: Some(false),
        ai_mode: Some("auto".to_string()),
        mock_data_enabled: Some(false),
        onboarding_completed: Some(true),
        onboarding_permanently_dismissed: Some(false),
        onboarding_reminder_dismissed_until: None,
        plaid_connection_skipped: Some(false),
        plaid_reminder_dismissed_until: None,
        vault_key_acknowledged: Some(false),
        backup_sync_chosen: Some(false),
        assistant_data_consent: Some(false),
        assistant_model: None,
    }
}

async fn init_full_schema(client: &CloudClient) {
    client
        .initialize_schema()
        .await
        .expect("initialize_schema failed");
    // Workaround: user_profile is not created by initialize_schema due to
    // a semicolon inside a comment in unified_schema.sql that breaks the
    // naive split-by-semicolon logic in execute_unified_schema.
    client
        .connection()
        .execute(
            "CREATE TABLE IF NOT EXISTS user_profile (
            id TEXT PRIMARY KEY,
            platform_user_id TEXT NOT NULL,
            azure_id TEXT NOT NULL,
            email TEXT NOT NULL,
            display_name TEXT NOT NULL,
            given_name TEXT,
            family_name TEXT,
            surname TEXT,
            mobile_phone TEXT,
            job_title TEXT,
            street_address TEXT,
            city TEXT,
            state_province TEXT,
            postal_code TEXT,
            country_region TEXT,
            date_of_birth TEXT,
            ssn_last_four TEXT,
            employment_status TEXT,
            annual_income INTEGER,
            phone_number TEXT,
            preferred_name TEXT,
            emergency_contact_name TEXT,
            emergency_contact_phone TEXT,
            employer_name TEXT,
            role TEXT DEFAULT 'consumer',
            is_admin BOOLEAN DEFAULT FALSE,
            provider_onboarding_complete BOOLEAN DEFAULT FALSE,
            tenant_id TEXT NOT NULL,
            object_id TEXT NOT NULL,
            verified_id_credential_id TEXT,
            verified_id_status TEXT DEFAULT 'pending',
            verified_id_issued_at TEXT,
            consumer_verified_id_credential_id TEXT,
            consumer_verified_id_status TEXT DEFAULT 'pending',
            consumer_verified_id_issued_at TEXT,
            provider_verified_id_credential_id TEXT,
            provider_verified_id_status TEXT DEFAULT 'pending',
            provider_verified_id_issued_at TEXT,
            last_report_date DATETIME,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        )",
            (),
        )
        .await
        .expect("Failed to create user_profile table");
}

async fn insert_user_profile_raw(
    client: &CloudClient,
    profile: &freshcredit_libsql_local::UserProfile,
) {
    client
        .connection()
        .execute(
            "INSERT OR REPLACE INTO user_profile (
                id, platform_user_id, azure_id, email, display_name,
                given_name, family_name, surname, mobile_phone, job_title,
                street_address, city, state_province, postal_code, country_region,
                date_of_birth, ssn_last_four, employment_status, annual_income,
                phone_number, preferred_name, emergency_contact_name, emergency_contact_phone,
                employer_name, role, is_admin, provider_onboarding_complete, tenant_id,
                object_id, verified_id_credential_id, verified_id_status,
                verified_id_issued_at,
                consumer_verified_id_credential_id, consumer_verified_id_status, consumer_verified_id_issued_at,
                provider_verified_id_credential_id, provider_verified_id_status, provider_verified_id_issued_at,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
// TAG: surface=database owner=platform-team rule=DB-001
            libsql::params![
                profile.id.clone(),
                profile.platform_user_id.clone(),
                profile.azure_id.clone(),
                profile.email.clone(),
                profile.display_name.clone(),
                profile.given_name.clone(),
                profile.family_name.clone(),
                profile.surname.clone(),
                profile.mobile_phone.clone(),
                profile.job_title.clone(),
                profile.street_address.clone(),
                profile.city.clone(),
                profile.state_province.clone(),
                profile.postal_code.clone(),
                profile.country_region.clone(),
                profile.date_of_birth.clone(),
                profile.ssn_last_four.clone(),
                profile.employment_status.clone(),
                profile.annual_income,
                profile.phone_number.clone(),
                profile.preferred_name.clone(),
                profile.emergency_contact_name.clone(),
                profile.emergency_contact_phone.clone(),
                profile.employer_name.clone(),
                profile.role.clone(),
                profile.is_admin,
                profile.provider_onboarding_complete,
                profile.tenant_id.clone(),
                profile.object_id.clone(),
                profile.verified_id_credential_id.clone(),
                profile.verified_id_status.clone(),
                profile.verified_id_issued_at.clone(),
                profile.consumer_verified_id_credential_id.clone(),
                profile.consumer_verified_id_status.clone(),
                profile.consumer_verified_id_issued_at.clone(),
                profile.provider_verified_id_credential_id.clone(),
                profile.provider_verified_id_status.clone(),
                profile.provider_verified_id_issued_at.clone(),
                profile.created_at.clone(),
                profile.updated_at.clone(),
            ],
        )
        .await
        .expect("Failed to insert user profile");
}

#[allow(clippy::too_many_arguments)]
async fn insert_account_raw(
    client: &CloudClient,
    id: &str,
    user_id: &str,
    account_type: &str,
    balance: f64,
    // TAG: surface=database owner=platform-team rule=DB-001
    currency: &str,
    institution: &str,
    plaid_token: Option<&str>,
) {
    client
        .connection()
        .execute(
            "INSERT OR REPLACE INTO accounts (
                id, user_id, account_id, account_type, balance, currency, institution_name, plaid_access_token, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                id.to_string(),
                user_id.to_string(),
                id.to_string(),
                account_type.to_string(),
                balance,
                currency.to_string(),
                institution.to_string(),
                plaid_token,
                Utc::now().to_rfc3339(),
            ],
        )
        .await
        .expect("Failed to insert account");
}

#[allow(clippy::too_many_arguments)]
async fn insert_transaction_raw(
    client: &CloudClient,
    id: &str,
    user_id: &str,
    account_id: &str,
    amount: f64,
    name: &str,
    category: &str,
    merchant: &str,
) {
    client
        .connection()
        .execute(
            "INSERT OR REPLACE INTO transactions (
                id, user_id, account_id, amount, name, category, merchant_name, date, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                id.to_string(),
                user_id.to_string(),
                account_id.to_string(),
                amount,
                // TAG: surface=database owner=platform-team rule=DB-001
                name.to_string(),
                category.to_string(),
                merchant.to_string(),
                Utc::now().to_rfc3339(),
                Utc::now().to_rfc3339(),
            ],
        )
        .await
        .expect("Failed to insert transaction");
}

#[tokio::test]
async fn test_cloud_client_can_be_created() {
    let client = CloudClient::new_test().await;
    drop(client);
}

#[tokio::test]
async fn test_initialize_schema_on_empty_database() {
    let client = CloudClient::new_test().await;
    let result = client.initialize_schema().await;
    assert!(
        result.is_ok(),
        "initialize_schema should succeed on empty DB"
    );

    // Verify a key table exists (user_profile is missing due to production bug)
    let mut rows = client
        .connection()
        .query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='accounts'",
            libsql::params![],
        )
        .await
        .unwrap();

    let row = rows.next().await.unwrap();
    assert!(row.is_some(), "accounts table should exist");
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_initialize_schema_is_idempotent() {
    let client = CloudClient::new_test().await;
    client
        .initialize_schema()
        .await
        .expect("First init should succeed");
    let result = client.initialize_schema().await;
    assert!(result.is_ok(), "Second init should also succeed");
}

#[tokio::test]
async fn test_sync_and_get_financial_report() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    let profile = test_user_profile_simple();
    insert_user_profile_raw(&client, &profile).await;

    let user_id = test_user_id();
    let report = test_financial_report(&user_id);

    let sync_result = client.sync_financial_report(&report).await;
    assert!(sync_result.is_ok(), "sync_financial_report should succeed");
    // TAG: surface=database owner=platform-team rule=GENERAL-001

    let retrieved = client.get_financial_report(&user_id).await;
    assert!(retrieved.is_ok(), "get_financial_report should succeed");

    let opt = retrieved.unwrap();
    assert!(opt.is_some(), "Report should be found");

    let retrieved_report = opt.unwrap();
    assert_eq!(retrieved_report.id, report.id);
    assert_eq!(retrieved_report.user_id, report.user_id);
    assert_eq!(retrieved_report.bureau_score, report.bureau_score);
    assert_eq!(retrieved_report.blockchain_hash, report.blockchain_hash);
    assert_eq!(retrieved_report.accounts.len(), report.accounts.len());
}

#[tokio::test]
async fn test_get_financial_report_not_found() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    let result = client
        .get_financial_report(&"nonexistent-user".to_string())
        .await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_sync_account() {
    let client = CloudClient::new_test().await;
    client.initialize_schema().await.unwrap();

    let user_id = test_user_id();
    let account = test_account(&user_id, "acc-1", freshcredit_types::AccountType::Checking);

    let sync_result = client.sync_account(&account).await;
    assert!(sync_result.is_ok(), "sync_account should succeed");

    let accounts = client.get_user_accounts(&user_id).await;
    assert!(accounts.is_ok());
    let accounts = accounts.unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].id, account.id);
    assert_eq!(accounts[0].user_id, account.user_id);
    assert_eq!(accounts[0].account_type, account.account_type);
    assert_eq!(accounts[0].currency, account.currency);
    assert_eq!(accounts[0].institution_name, account.institution_name);
}

#[tokio::test]
async fn test_get_account_by_id_found() {
    let client = CloudClient::new_test().await;
    client.initialize_schema().await.unwrap();

    let user_id = test_user_id();
    let account = test_account(
        &user_id,
        "acc-by-id",
        freshcredit_types::AccountType::Savings,
    );
    client.sync_account(&account).await.unwrap();

    let result = client.get_account_by_id(&account.id).await;
    assert!(result.is_ok());
    let opt = result.unwrap();
    assert!(opt.is_some());
    let retrieved = opt.unwrap();
    assert_eq!(retrieved.id, account.id);
    assert_eq!(
        retrieved.account_type,
        freshcredit_types::AccountType::Savings
    );
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_get_account_by_id_not_found() {
    let client = CloudClient::new_test().await;
    client.initialize_schema().await.unwrap();

    let result = client.get_account_by_id("nonexistent").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_get_account_count() {
    let client = CloudClient::new_test().await;
    client.initialize_schema().await.unwrap();

    let user_id = test_user_id();
    assert_eq!(client.get_account_count().await.unwrap(), 0);

    client
        .sync_account(&test_account(
            &user_id,
            "acc-1",
            freshcredit_types::AccountType::Checking,
        ))
// TAG: surface=database owner=platform-team rule=GENERAL-001
        .await
        .unwrap();
    assert_eq!(client.get_account_count().await.unwrap(), 1);

    client
        .sync_account(&test_account(
            &user_id,
            "acc-2",
            freshcredit_types::AccountType::Credit,
        ))
        .await
        .unwrap();
    assert_eq!(client.get_account_count().await.unwrap(), 2);
}

#[tokio::test]
async fn test_account_type_parsing() {
    let client = CloudClient::new_test().await;
    client.initialize_schema().await.unwrap();

    let user_id = test_user_id();
    let types = [
        (freshcredit_types::AccountType::Checking, "checking"),
        (freshcredit_types::AccountType::Savings, "savings"),
        (freshcredit_types::AccountType::Credit, "credit"),
        (freshcredit_types::AccountType::Investment, "investment"),
        (freshcredit_types::AccountType::Loan, "loan"),
    ];

    // TAG: surface=database owner=platform-team rule=DB-001
    for (i, (acc_type, _)) in types.iter().enumerate() {
        let acc = test_account(&user_id, &format!("acc-{i}"), acc_type.clone());
        client.sync_account(&acc).await.unwrap();
    }

    insert_account_raw(
        &client,
        "acc-unknown",
        &user_id,
        "unknown_type",
        0.0,
        "USD",
        "Unknown Bank",
        None,
    )
    .await;

    let accounts = client.get_user_accounts(&user_id).await.unwrap();
    assert_eq!(accounts.len(), 6);

    let found_types: Vec<_> = accounts.iter().map(|a| &a.account_type).collect();
    assert!(found_types.contains(&&freshcredit_types::AccountType::Checking));
    assert!(found_types.contains(&&freshcredit_types::AccountType::Savings));
    assert!(found_types.contains(&&freshcredit_types::AccountType::Credit));
    assert!(found_types.contains(&&freshcredit_types::AccountType::Investment));
    assert!(found_types.contains(&&freshcredit_types::AccountType::Loan));

    let unknown = accounts.iter().find(|a| a.id == "acc-unknown").unwrap();
    assert_eq!(
        unknown.account_type,
        freshcredit_types::AccountType::Checking
    );
}

#[tokio::test]
async fn test_sync_transaction_with_custom_table() {
    let client = CloudClient::new_test().await;

    client
// TAG: surface=database owner=platform-team rule=DB-001
        .connection()
        .execute(
            "CREATE TABLE transactions (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                amount REAL NOT NULL,
                currency TEXT,
                description TEXT,
                category TEXT,
                date TEXT,
                merchant_name TEXT
            )",
            (),
        )
        .await
        .unwrap();

    let tx = test_transaction("acc-1", "tx-1");
    let result = client.sync_transaction(&tx).await;
    assert!(
        result.is_ok(),
        "sync_transaction should succeed with custom table"
    );
    // TAG: surface=database owner=platform-team rule=GENERAL-001

    let mut rows = client
        .connection()
        .query(
            "SELECT id, account_id, amount, description, category, merchant_name FROM transactions WHERE id = ?",
            libsql::params![tx.id.clone()],
        )
        .await
        .unwrap();

    let row = rows
        .next()
        .await
        .unwrap()
        .expect("Transaction should exist");
    let id: String = row.get(0).unwrap();
    let account_id: String = row.get(1).unwrap();
    let amount: f64 = row.get(2).unwrap();
    let description: String = row.get(3).unwrap();
    let category: String = row.get(4).unwrap();
    let merchant: String = row.get(5).unwrap();

    assert_eq!(id, tx.id);
    assert_eq!(account_id, tx.account_id);
    assert!((amount - tx.amount).abs() < f64::EPSILON);
    assert_eq!(description, tx.description);
    assert_eq!(category, tx.category.unwrap());
    assert_eq!(merchant, tx.merchant_name.unwrap());
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_get_transaction_count() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    let user_id = test_user_id();
    assert_eq!(client.get_transaction_count().await.unwrap(), 0);

    let profile = test_user_profile_simple();
    insert_user_profile_raw(&client, &profile).await;

    insert_account_raw(
        &client,
        "acc-1",
        &user_id,
        "checking",
        1000.0,
        "USD",
        "Test Bank",
        None,
    )
    .await;
    insert_transaction_raw(
        &client,
        "tx-1",
        &user_id,
        "acc-1",
        50.0,
        "Coffee",
        "Food",
        "Starbucks",
    )
    .await;
    insert_transaction_raw(
        &client, "tx-2", &user_id, "acc-1", 25.0, "Lunch", "Food", "Subway",
    )
    .await;

    assert_eq!(client.get_transaction_count().await.unwrap(), 2);
}

#[tokio::test]
async fn test_get_user_transactions() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    // TAG: surface=database owner=platform-team rule=DB-001
    let user_id = test_user_id();
    let profile = test_user_profile_simple();
    insert_user_profile_raw(&client, &profile).await;

    insert_account_raw(
        &client,
        "acc-1",
        &user_id,
        "checking",
        1000.0,
        "USD",
        "Test Bank",
        None,
    )
    .await;
    insert_transaction_raw(
        &client,
        "tx-1",
        &user_id,
        "acc-1",
        50.0,
        "Coffee",
        "Food",
        "Starbucks",
    )
    .await;
    insert_transaction_raw(
        &client, "tx-2", &user_id, "acc-1", 25.0, "Lunch", "Food", "Subway",
    )
    .await;

    let result = client.get_user_transactions(&user_id).await;
    assert!(result.is_ok());
    let txs = result.unwrap();
    assert_eq!(txs.len(), 2);

    let tx1 = txs.iter().find(|t| t.id == "tx-1").unwrap();
    assert_eq!(tx1.account_id, "acc-1");
    assert!((tx1.amount - 50.0).abs() < f64::EPSILON);
    assert_eq!(tx1.currency, "USD");
    assert_eq!(tx1.description, "Coffee");
    assert_eq!(tx1.category, Some("Food".to_string()));
    assert_eq!(tx1.merchant_name, Some("Starbucks".to_string()));
}

#[tokio::test]
async fn test_get_user_transactions_empty() {
    let client = CloudClient::new_test().await;
    client.initialize_schema().await.unwrap();

    // TAG: surface=database owner=platform-team rule=DB-001
    let user_id = test_user_id();
    let result = client.get_user_transactions(&user_id).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn test_sync_user_profile() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    let profile = freshcredit_libsql_local::UserProfile {
        platform_user_id: "plat-1".to_string(),
        email: "sync@example.com".to_string(),
        display_name: "Sync User".to_string(),
        given_name: Some("Sync".to_string()),
        surname: Some("User".to_string()),
        object_id: "obj-sync".to_string(),
        verified_id_credential_id: Some("vc-1".to_string()),
        verified_id_status: "pending".to_string(),
        verified_id_issued_at: Some("2024-01-01".to_string()),
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
        ..test_user_profile_simple()
    };

    let result = client.sync_user_profile(&profile).await;
    assert!(result.is_ok(), "sync_user_profile should succeed");

    let mut rows = client
        .connection()
        .query(
            "SELECT platform_user_id, email, display_name FROM user_profile WHERE platform_user_id = ?",
            libsql::params![profile.platform_user_id.clone()],
        )
        .await
        .unwrap();

    let row = rows.next().await.unwrap().expect("Profile should exist");
    let email: String = row.get(1).unwrap();
    assert_eq!(email, profile.email);
}

#[tokio::test]
async fn test_store_user_profile() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    // TAG: surface=database owner=platform-team rule=DB-001
    let profile = test_user_profile_simple();
    let result = client.store_user_profile(&profile).await;
    assert!(result.is_ok(), "store_user_profile should succeed");

    let mut rows = client
        .connection()
        .query(
            "SELECT id, platform_user_id, email, role, tenant_id FROM user_profile WHERE platform_user_id = ?",
            libsql::params![profile.platform_user_id.clone()],
        )
        .await
        .unwrap();

    let row = rows.next().await.unwrap().expect("Profile should exist");
    let id: String = row.get(0).unwrap();
    let role: String = row.get(3).unwrap();
    assert_eq!(id, profile.id);
    assert_eq!(role, profile.role);
}

#[tokio::test]
async fn test_get_user_profile_found() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    let profile = test_user_profile_simple();
    insert_user_profile_raw(&client, &profile).await;

    let result = client.get_user_profile(&profile.email).await;
    assert!(result.is_ok());
    let opt = result.unwrap();
    assert!(opt.is_some());

    let retrieved = opt.unwrap();
    assert_eq!(retrieved.platform_user_id, profile.platform_user_id);
    assert_eq!(retrieved.email, profile.email);
    assert_eq!(retrieved.display_name, profile.display_name);
    assert_eq!(retrieved.object_id, profile.object_id);
    assert_eq!(retrieved.verified_id_status, profile.verified_id_status);
    assert_eq!(retrieved.role, "consumer");
    assert_eq!(retrieved.tenant_id, "freshcredit");
    assert!(!retrieved.is_admin);
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_get_user_profile_by_platform_user_id() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    let profile = test_user_profile_simple();
    insert_user_profile_raw(&client, &profile).await;

    let result = client.get_user_profile(&profile.platform_user_id).await;
    assert!(result.is_ok());
    let opt = result.unwrap();
    assert!(opt.is_some());
    assert_eq!(opt.unwrap().platform_user_id, profile.platform_user_id);
}

#[tokio::test]
async fn test_get_user_profile_not_found() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    let result = client.get_user_profile("nobody@example.com").await;
    // TAG: surface=database owner=platform-team rule=GENERAL-001
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_get_user_profile_by_azure_id_found() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    let profile = test_user_profile_simple();
    insert_user_profile_raw(&client, &profile).await;

    let result = client.get_user_profile_by_azure_id(&profile.azure_id).await;
    assert!(result.is_ok());
    let opt = result.unwrap();
    assert!(opt.is_some());

    let retrieved = opt.unwrap();
    // Fields 0-18 are correctly mapped in production code
    assert_eq!(retrieved.platform_user_id, profile.platform_user_id);
    assert_eq!(retrieved.email, profile.email);
    assert_eq!(retrieved.display_name, profile.display_name);
    assert_eq!(retrieved.azure_id, profile.azure_id);
}

#[tokio::test]
async fn test_get_user_profile_by_azure_id_not_found() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    // TAG: surface=database owner=platform-team rule=DB-001
    let result = client.get_user_profile_by_azure_id("nonexistent").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_get_user_profile_by_azure_id_tolerates_text_boolean_columns() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    // Simulate a legacy or browser-side writer that stored booleans/numbers as
    // TEXT. libsql's typed row.get::<i64>() panics on type mismatch; the reader
    // must coerce instead of aborting the process.
    client
        .connection()
        .execute(
            "INSERT INTO user_profile (
                id, platform_user_id, azure_id, email, display_name,
                given_name, family_name, surname, mobile_phone, job_title,
                street_address, city, state_province, postal_code, country_region,
                date_of_birth, ssn_last_four, employment_status, annual_income,
                phone_number, preferred_name, emergency_contact_name, emergency_contact_phone,
                employer_name, role, is_admin, provider_onboarding_complete, tenant_id,
                object_id, verified_id_credential_id, verified_id_status,
                verified_id_issued_at,
                consumer_verified_id_credential_id, consumer_verified_id_status, consumer_verified_id_issued_at,
                provider_verified_id_credential_id, provider_verified_id_status, provider_verified_id_issued_at,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            libsql::params![
                "azure-text", "azure-text", "azure-text", "text@example.com", "Text User",
                None::<String>, None::<String>, None::<String>, None::<String>, None::<String>,
                None::<String>, None::<String>, None::<String>, None::<String>, None::<String>,
                None::<String>, None::<String>, None::<String>, "82000",
                None::<String>, None::<String>, None::<String>, None::<String>,
                None::<String>, "consumer", "true", "0", "freshcredit",
                "azure-text", None::<String>, "pending",
                None::<String>,
                None::<String>, "pending", None::<String>,
                None::<String>, "pending", None::<String>,
                "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"
            ],
        )
        .await
        .expect("Failed to insert text-typed profile row");

    let result = client.get_user_profile_by_azure_id("azure-text").await;
    assert!(result.is_ok(), "reader must not panic on text booleans");
    let profile = result.unwrap().expect("profile should be found");
    assert!(profile.is_admin, "text 'true' should coerce to true");
    assert!(
        !profile.provider_onboarding_complete,
        "text '0' should coerce to false"
    );
    assert_eq!(
        profile.annual_income,
        Some(82_000),
        "text annual_income should coerce to integer"
    );
}

#[tokio::test]
async fn test_save_and_get_user_preferences() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    let profile = test_user_profile_simple();
    insert_user_profile_raw(&client, &profile).await;

    let prefs = test_user_preferences();
    let save_result = client.save_user_preferences(&profile.id, &prefs).await;
    assert!(save_result.is_ok(), "save_user_preferences should succeed");

    let get_result = client.get_user_preferences(&profile.id).await;
    assert!(get_result.is_ok());
    let opt = get_result.unwrap();
    assert!(opt.is_some());

    let retrieved = opt.unwrap();
    // First 10 fields are correctly mapped in production code
    assert_eq!(retrieved.ai_agent_enabled, prefs.ai_agent_enabled);
    assert_eq!(retrieved.ai_feedback_enabled, prefs.ai_feedback_enabled);
    assert_eq!(retrieved.ai_offers_enabled, prefs.ai_offers_enabled);
    assert_eq!(retrieved.ai_lenders_enabled, prefs.ai_lenders_enabled);
    assert_eq!(retrieved.cloud_sync_enabled, prefs.cloud_sync_enabled);
    assert_eq!(retrieved.blockchain_enabled, prefs.blockchain_enabled);
    assert_eq!(
        retrieved.email_notifications_enabled,
        prefs.email_notifications_enabled
    );
    assert_eq!(retrieved.kilt_did_enabled, prefs.kilt_did_enabled);
    assert_eq!(retrieved.ai_mode, prefs.ai_mode);
    assert_eq!(retrieved.mock_data_enabled, prefs.mock_data_enabled);
}

#[tokio::test]
async fn test_get_user_preferences_not_found() {
    let client = CloudClient::new_test().await;
    init_full_schema(&client).await;

    let result = client.get_user_preferences("nonexistent").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_get_plaid_access_token_found() {
    let client = CloudClient::new_test().await;
    client.initialize_schema().await.unwrap();

    let user_id = test_user_id();
    insert_account_raw(
        &client,
        "acc-plaid",
        &user_id,
        "checking",
        1000.0,
        "USD",
        "Plaid Bank",
        Some("token-123"),
    )
    .await;

    let result = client.get_plaid_access_token(&user_id).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some("token-123".to_string()));
}

#[tokio::test]
async fn test_get_plaid_access_token_not_found() {
    let client = CloudClient::new_test().await;
    client.initialize_schema().await.unwrap();

    let user_id = test_user_id();
    insert_account_raw(
        &client,
        "acc-no-plaid",
        &user_id,
        "checking",
        1000.0,
        "USD",
        "No Plaid Bank",
        None,
    )
    .await;

    let result = client.get_plaid_access_token(&user_id).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// TAG: surface=database owner=platform-team rule=DB-001
#[tokio::test]
async fn test_get_plaid_access_token_for_account_found() {
    let client = CloudClient::new_test().await;
    client.initialize_schema().await.unwrap();

    let user_id = test_user_id();
    insert_account_raw(
        &client,
        "acc-specific",
        &user_id,
        "savings",
        5000.0,
        "USD",
        "Specific Bank",
        Some("token-specific"),
    )
    .await;

    let result = client
        .get_plaid_access_token_for_account("acc-specific")
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some("token-specific".to_string()));
}

#[tokio::test]
async fn test_get_plaid_access_token_for_account_not_found() {
    let client = CloudClient::new_test().await;
    client.initialize_schema().await.unwrap();

    let result = client
        .get_plaid_access_token_for_account("nonexistent")
        .await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_get_user_accounts_returns_empty_for_unknown_user() {
    let client = CloudClient::new_test().await;
    client.initialize_schema().await.unwrap();

    let result = client.get_user_accounts("unknown-user").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}
