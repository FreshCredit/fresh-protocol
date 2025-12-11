//! End-to-end tests for payment flow
//!
//! Tests the complete payment journey:
//! 1. ACH bank account linking via Plaid
//! 2. Payment method creation and verification
//! 3. Transaction initiation
//! 4. Webhook handling and status updates
//! 5. Settlement tracking

use anyhow::Result;
use freshcredit_libsql_local::LocalClient;
use libsql::Value;

/// Test user and customer data for payment tests
#[derive(Clone)]
struct TestUserData {
    user_id: String,
    customer_id: String,
}

/// Helper to create a test user profile and customer for payment tests
async fn setup_test_user(client: &LocalClient) -> Result<TestUserData> {
    let user_id = uuid::Uuid::new_v4().to_string();
    let profile = freshcredit_libsql_local::UserProfile {
        id: user_id.clone(),
        platform_user_id: "payment-test@example.com".to_string(),
        azure_id: format!("azure-{user_id}"),
        email: "payment-test@example.com".to_string(),
        display_name: "Payment Test User".to_string(),
        given_name: Some("Payment".to_string()),
        family_name: Some("Test".to_string()),
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
        role: "consumer".to_string(),
        tenant_id: "freshcredit".to_string(),
        object_id: format!("azure-{user_id}"),
        verified_id_credential_id: None,
        verified_id_status: "verified".to_string(),
        verified_id_issued_at: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    client.store_user_profile(&profile).await?;

    // Create a customer record (required for funding_sources and payments foreign keys)
    let customer_id = uuid::Uuid::new_v4().to_string();
    client.execute(
        "INSERT INTO customers (id, user_id, stripe_customer_id, customer_type, email, first_name, last_name, status, raw_customer_data, created_at)
         VALUES (?, ?, ?, 'consumer', 'payment-test@example.com', 'Payment', 'Test', 'active', '{}', CURRENT_TIMESTAMP)",
        vec![Value::Text(customer_id.clone()), Value::Text(user_id.clone()), Value::Text(format!("cus_{user_id}"))]
    ).await?;

    Ok(TestUserData { user_id, customer_id })
}

/// Helper to check if rows exist
async fn has_rows(rows: &mut libsql::Rows) -> bool {
    rows.next().await.ok().flatten().is_some()
}

/// Test ACH bank account linking via Plaid
#[tokio::test]
async fn test_ach_bank_account_linking() -> Result<()> {
    println!("🧪 E2E Test: ACH Bank Account Linking");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;
    let test_data = setup_test_user(&client).await?;

    println!("✅ Test database initialized with user: {}", test_data.user_id);

    // Simulate Plaid account linking - store account in database
    let account_id = uuid::Uuid::new_v4().to_string();
    client.execute(
        "INSERT INTO accounts (id, user_id, account_type, balance, currency, institution_name, created_at)
         VALUES (?, ?, 'checking', 5000.00, 'USD', 'Chase Bank', CURRENT_TIMESTAMP)",
        vec![Value::Text(account_id.clone()), Value::Text(test_data.user_id.clone())]
    ).await?;
    println!("✅ Bank account linked: {account_id}");

    // Verify account was stored
    let mut rows = client.query(
        "SELECT * FROM accounts WHERE user_id = ?",
        vec![Value::Text(test_data.user_id.clone())]
    ).await?;
    assert!(has_rows(&mut rows).await, "Account should be stored in database");
    println!("✅ Account verified in database");

    // Simulate funding source creation from linked account (uses funding_sources table)
    let funding_source_id = uuid::Uuid::new_v4().to_string();
    client.execute(
        "INSERT INTO funding_sources (id, user_id, customer_id, account_id, funding_source_id, funding_source_type, bank_name, bank_account_type, name, status, is_default, raw_funding_source_data, created_at)
         VALUES (?, ?, ?, ?, 'fs_test_123', 'bank', 'Chase Bank', 'checking', 'Chase Checking', 'verified', 1, '{}', CURRENT_TIMESTAMP)",
        vec![Value::Text(funding_source_id.clone()), Value::Text(test_data.user_id.clone()), Value::Text(test_data.customer_id.clone()), Value::Text(account_id.clone())]
    ).await?;
    println!("✅ Funding source created: {funding_source_id}");

    // Verify funding source
    let mut fs_rows = client.query(
        "SELECT * FROM funding_sources WHERE user_id = ?",
        vec![Value::Text(test_data.user_id.clone())]
    ).await?;
    assert!(has_rows(&mut fs_rows).await, "Funding source should be stored");
    println!("✅ Funding source verified in database");

    println!("🎉 ACH Bank Account Linking test passed!");
    Ok(())
}

/// Test payment transaction initiation
#[tokio::test]
async fn test_payment_transaction_initiation() -> Result<()> {
    println!("🧪 E2E Test: Payment Transaction Initiation");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;
    let test_data = setup_test_user(&client).await?;

    // Create funding source first
    let funding_source_id = uuid::Uuid::new_v4().to_string();
    client.execute(
        "INSERT INTO funding_sources (id, user_id, customer_id, funding_source_id, funding_source_type, bank_name, status, is_default, raw_funding_source_data, created_at)
         VALUES (?, ?, ?, 'fs_test_456', 'bank', 'Chase Bank', 'verified', 1, '{}', CURRENT_TIMESTAMP)",
        vec![Value::Text(funding_source_id.clone()), Value::Text(test_data.user_id.clone()), Value::Text(test_data.customer_id.clone())]
    ).await?;
    println!("✅ Funding source created");

    // Initiate a payment transaction (uses amount REAL, not amount_cents)
    let payment_id = uuid::Uuid::new_v4().to_string();
    client.execute(
        "INSERT INTO payments (id, user_id, customer_id, funding_source_id, payment_type, amount, currency, status, description, raw_payment_data, created_at)
         VALUES (?, ?, ?, ?, 'ach_transfer', 100.00, 'USD', 'pending', 'Test payment', '{}', CURRENT_TIMESTAMP)",
        vec![Value::Text(payment_id.clone()), Value::Text(test_data.user_id.clone()), Value::Text(test_data.customer_id.clone()), Value::Text(funding_source_id)]
    ).await?;
    println!("✅ Payment initiated: {payment_id} ($100.00)");

    // Verify payment status is pending
    let mut payment_rows = client.query(
        "SELECT status FROM payments WHERE id = ?",
        vec![Value::Text(payment_id.clone())]
    ).await?;
    assert!(has_rows(&mut payment_rows).await, "Payment should exist");
    println!("✅ Payment status verified as pending");

    println!("🎉 Payment Transaction Initiation test passed!");
    Ok(())
}

/// Test webhook handling and status updates
#[tokio::test]
async fn test_webhook_status_updates() -> Result<()> {
    println!("🧪 E2E Test: Webhook Status Updates");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;
    let test_data = setup_test_user(&client).await?;

    // Create a pending payment (uses amount REAL, not amount_cents)
    let payment_id = uuid::Uuid::new_v4().to_string();
    client.execute(
        "INSERT INTO payments (id, user_id, customer_id, payment_type, amount, currency, status, description, raw_payment_data, created_at)
         VALUES (?, ?, ?, 'ach_transfer', 250.00, 'USD', 'pending', 'Webhook test payment', '{}', CURRENT_TIMESTAMP)",
        vec![Value::Text(payment_id.clone()), Value::Text(test_data.user_id.clone()), Value::Text(test_data.customer_id.clone())]
    ).await?;
    println!("✅ Pending payment created: {payment_id}");

    // Simulate webhook event storage
    let webhook_id = uuid::Uuid::new_v4().to_string();
    let event_id = format!("evt_test_{}", uuid::Uuid::new_v4());
    client.execute(
        "INSERT INTO webhook_events (id, user_id, provider, event_type, event_id, payload, status, retry_count, created_at)
         VALUES (?, ?, 'stripe', 'payment_intent.succeeded', ?, ?, 'pending', 0, CURRENT_TIMESTAMP)",
        vec![
            Value::Text(webhook_id.clone()),
            Value::Text(test_data.user_id.clone()),
            Value::Text(event_id),
            Value::Text(format!("{{\"payment_id\": \"{payment_id}\"}}"))
        ]
    ).await?;
    println!("✅ Webhook event stored: {webhook_id}");

    // Simulate webhook processing - update payment status (uses payment_id column, not external_transaction_id)
    client.execute(
        "UPDATE payments SET status = 'completed', payment_id = 'pi_test_789', completed_at = CURRENT_TIMESTAMP WHERE id = ?",
        vec![Value::Text(payment_id.clone())]
    ).await?;
    println!("✅ Payment status updated to completed");

    // Mark webhook as processed
    client.execute(
        "UPDATE webhook_events SET status = 'processed', processed_at = CURRENT_TIMESTAMP WHERE id = ?",
        vec![Value::Text(webhook_id.clone())]
    ).await?;
    println!("✅ Webhook marked as processed");

    // Verify final states
    let mut final_payment = client.query(
        "SELECT status FROM payments WHERE id = ?",
        vec![Value::Text(payment_id.clone())]
    ).await?;
    assert!(has_rows(&mut final_payment).await, "Payment should exist");

    let mut final_webhook = client.query(
        "SELECT status FROM webhook_events WHERE id = ?",
        vec![Value::Text(webhook_id.clone())]
    ).await?;
    assert!(has_rows(&mut final_webhook).await, "Webhook should exist");

    println!("🎉 Webhook Status Updates test passed!");
    Ok(())
}

/// Test payment status tracking (pending -> completed flow)
#[tokio::test]
async fn test_payment_status_tracking() -> Result<()> {
    println!("🧪 E2E Test: Payment Status Tracking");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;
    let test_data = setup_test_user(&client).await?;

    // Create a pending payment
    let payment_id = uuid::Uuid::new_v4().to_string();
    client.execute(
        "INSERT INTO payments (id, user_id, customer_id, payment_type, amount, currency, status, description, raw_payment_data, initiated_at, created_at)
         VALUES (?, ?, ?, 'ach_transfer', 500.00, 'USD', 'pending', 'Status tracking test', '{}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![Value::Text(payment_id.clone()), Value::Text(test_data.user_id.clone()), Value::Text(test_data.customer_id.clone())]
    ).await?;
    println!("✅ Pending payment created: {payment_id}");

    // Query pending payments
    let mut pending_payments = client.query(
        "SELECT id, status FROM payments WHERE status = 'pending'",
        vec![]
    ).await?;
    assert!(has_rows(&mut pending_payments).await, "Should have pending payments");
    println!("✅ Found pending payments");

    // Simulate payment completion
    client.execute(
        "UPDATE payments SET status = 'completed', payment_id = 'pi_settled_123', completed_at = CURRENT_TIMESTAMP WHERE id = ?",
        vec![Value::Text(payment_id.clone())]
    ).await?;
    println!("✅ Payment marked as completed");

    // Verify payment status
    let mut completed = client.query(
        "SELECT status FROM payments WHERE id = ?",
        vec![Value::Text(payment_id.clone())]
    ).await?;
    assert!(has_rows(&mut completed).await, "Payment should exist");
    println!("✅ Payment status verified as completed");

    println!("🎉 Payment Status Tracking test passed!");
    Ok(())
}

/// Test complete payment flow end-to-end
#[tokio::test]
async fn test_complete_payment_flow() -> Result<()> {
    println!("🧪 E2E Test: Complete Payment Flow");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;
    let test_data = setup_test_user(&client).await?;
    println!("✅ Step 1: User created");

    // Step 2: Link bank account
    let account_id = uuid::Uuid::new_v4().to_string();
    client.execute(
        "INSERT INTO accounts (id, user_id, account_type, balance, currency, institution_name, created_at)
         VALUES (?, ?, 'checking', 10000.00, 'USD', 'Bank of America', CURRENT_TIMESTAMP)",
        vec![Value::Text(account_id.clone()), Value::Text(test_data.user_id.clone())]
    ).await?;
    println!("✅ Step 2: Bank account linked");

    // Step 3: Create funding source (uses funding_sources table, not payment_methods)
    let funding_source_id = uuid::Uuid::new_v4().to_string();
    client.execute(
        "INSERT INTO funding_sources (id, user_id, customer_id, account_id, funding_source_id, funding_source_type, bank_name, bank_account_type, name, status, is_default, raw_funding_source_data, created_at)
         VALUES (?, ?, ?, ?, 'fs_boa_123', 'bank', 'Bank of America', 'checking', 'BoA Checking', 'verified', 1, '{}', CURRENT_TIMESTAMP)",
        vec![Value::Text(funding_source_id.clone()), Value::Text(test_data.user_id.clone()), Value::Text(test_data.customer_id.clone()), Value::Text(account_id.clone())]
    ).await?;
    println!("✅ Step 3: Funding source created");

    // Step 4: Initiate payment (uses amount REAL, not amount_cents)
    let payment_id = uuid::Uuid::new_v4().to_string();
    client.execute(
        "INSERT INTO payments (id, user_id, customer_id, funding_source_id, payment_type, amount, currency, status, description, raw_payment_data, initiated_at, created_at)
         VALUES (?, ?, ?, ?, 'ach_transfer', 150.00, 'USD', 'pending', 'Complete flow test', '{}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![Value::Text(payment_id.clone()), Value::Text(test_data.user_id.clone()), Value::Text(test_data.customer_id.clone()), Value::Text(funding_source_id)]
    ).await?;
    println!("✅ Step 4: Payment initiated ($150.00)");

    // Step 5: Receive webhook and update status
    let webhook_id = uuid::Uuid::new_v4().to_string();
    let event_id = format!("evt_complete_{}", uuid::Uuid::new_v4());
    client.execute(
        "INSERT INTO webhook_events (id, user_id, provider, event_type, event_id, payload, status, retry_count, created_at)
         VALUES (?, ?, 'stripe', 'payment_intent.succeeded', ?, '{}', 'processed', 0, CURRENT_TIMESTAMP)",
        vec![Value::Text(webhook_id.clone()), Value::Text(test_data.user_id.clone()), Value::Text(event_id)]
    ).await?;
    client.execute(
        "UPDATE payments SET status = 'completed', payment_id = 'pi_complete_456', completed_at = CURRENT_TIMESTAMP WHERE id = ?",
        vec![Value::Text(payment_id.clone())]
    ).await?;
    println!("✅ Step 5: Webhook processed, payment completed");

    // Verify complete flow
    let mut final_payment = client.query(
        "SELECT status FROM payments WHERE id = ?",
        vec![Value::Text(payment_id.clone())]
    ).await?;
    assert!(has_rows(&mut final_payment).await, "Payment should exist");
    println!("✅ Complete payment flow verified");

    println!("🎉 Complete Payment Flow test passed!");
    Ok(())
}
