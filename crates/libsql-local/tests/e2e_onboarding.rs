//! End-to-end tests for user onboarding flow
//!
//! Tests the complete onboarding journey:
//! 1. Entra ID authentication (mocked)
//! 2. Profile creation and personal info
//! 3. Role selection and upgrade
//! 4. Verified ID status transitions
//! 5. Onboarding completion

use anyhow::Result;
use freshcredit_libsql_local::{LocalClient, UserProfile};

/// Helper to create a test profile
fn create_test_profile(azure_id: &str, email: &str, display_name: &str) -> UserProfile {
    UserProfile {
        id: uuid::Uuid::new_v4().to_string(),
        platform_user_id: email.to_string(),
        azure_id: azure_id.to_string(),
        email: email.to_string(),
        display_name: display_name.to_string(),
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
        role: "consumer".to_string(),
        tenant_id: "freshcredit".to_string(),
        object_id: azure_id.to_string(),
        verified_id_credential_id: None,
        verified_id_status: "pending".to_string(),
        verified_id_issued_at: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Test user profile creation during onboarding
#[tokio::test]
async fn test_onboarding_profile_creation() -> Result<()> {
    println!("🧪 E2E Test: Onboarding Profile Creation");

    // Create in-memory test database
    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;

    println!("✅ Test database initialized");

    // Simulate user profile creation (post-authentication)
    let azure_id = "azure-oid-67890";
    let email = "testuser@example.com";
    let display_name = "Test User";

    let mut profile = create_test_profile(azure_id, email, display_name);
    profile.given_name = Some("Test".to_string());
    profile.family_name = Some("User".to_string());

    client.store_user_profile(&profile).await?;
    println!("✅ User profile stored");

    // Verify profile retrieval
    let retrieved = client.get_user_profile_by_azure_id(azure_id).await?;
    assert!(retrieved.is_some(), "Profile should be retrievable");

    let retrieved_profile = retrieved.unwrap();
    assert_eq!(retrieved_profile.email, email);
    assert_eq!(retrieved_profile.display_name, display_name);
    assert_eq!(retrieved_profile.role, "consumer");
    assert_eq!(retrieved_profile.verified_id_status, "pending");

    println!("✅ Profile verification passed");
    println!("✅ E2E Test: Onboarding Profile Creation - PASSED");

    Ok(())
}

/// Test onboarding role selection (consumer vs provider)
#[tokio::test]
async fn test_onboarding_role_selection() -> Result<()> {
    println!("🧪 E2E Test: Onboarding Role Selection");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;

    // Create initial consumer profile
    let azure_id = "azure-role-test-123";
    let mut profile = freshcredit_libsql_local::UserProfile {
        id: uuid::Uuid::new_v4().to_string(),
        platform_user_id: "roletest@example.com".to_string(),
        azure_id: azure_id.to_string(),
        email: "roletest@example.com".to_string(),
        display_name: "Role Test User".to_string(),
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
        role: "consumer".to_string(),
        tenant_id: "freshcredit".to_string(),
        object_id: azure_id.to_string(),
        verified_id_credential_id: None,
        verified_id_status: "pending".to_string(),
        verified_id_issued_at: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    client.store_user_profile(&profile).await?;
    println!("✅ Initial consumer profile created");

    // Simulate role upgrade to provider
    profile.role = "provider".to_string();
    profile.verified_id_status = "onboarding_complete".to_string();
    profile.updated_at = chrono::Utc::now().to_rfc3339();

    client.store_user_profile(&profile).await?;
    println!("✅ Profile upgraded to provider");

    // Verify role change persisted
    let retrieved = client
        .get_user_profile_by_azure_id(azure_id)
        .await?
        .unwrap();
    assert_eq!(retrieved.role, "provider");
    assert_eq!(retrieved.verified_id_status, "onboarding_complete");

    println!("✅ Role upgrade verification passed");
    println!("✅ E2E Test: Onboarding Role Selection - PASSED");

    Ok(())
}

/// Test verified ID status transitions during onboarding
#[tokio::test]
async fn test_onboarding_verified_id_transitions() -> Result<()> {
    println!("🧪 E2E Test: Verified ID Status Transitions");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;

    let azure_id = "azure-verified-id-test-456";
    let mut profile =
        create_test_profile(azure_id, "verifiedtest@example.com", "Verified Test User");

    // Initial state: pending
    assert_eq!(profile.verified_id_status, "pending");
    client.store_user_profile(&profile).await?;
    println!("✅ Initial profile created with pending status");

    // Transition 1: pending -> verification_requested
    profile.verified_id_status = "verification_requested".to_string();
    profile.updated_at = chrono::Utc::now().to_rfc3339();
    client.store_user_profile(&profile).await?;

    let retrieved = client
        .get_user_profile_by_azure_id(azure_id)
        .await?
        .unwrap();
    assert_eq!(retrieved.verified_id_status, "verification_requested");
    println!("✅ Transition: pending -> verification_requested");

    // Transition 2: verification_requested -> verified
    profile.verified_id_status = "verified".to_string();
    profile.verified_id_credential_id = Some("vc-credential-123".to_string());
    profile.verified_id_issued_at = Some(chrono::Utc::now().to_rfc3339());
    profile.updated_at = chrono::Utc::now().to_rfc3339();
    client.store_user_profile(&profile).await?;

    let retrieved = client
        .get_user_profile_by_azure_id(azure_id)
        .await?
        .unwrap();
    assert_eq!(retrieved.verified_id_status, "verified");
    assert!(retrieved.verified_id_credential_id.is_some());
    println!("✅ Transition: verification_requested -> verified");

    // Transition 3: verified -> onboarding_complete
    profile.verified_id_status = "onboarding_complete".to_string();
    profile.updated_at = chrono::Utc::now().to_rfc3339();
    client.store_user_profile(&profile).await?;

    let retrieved = client
        .get_user_profile_by_azure_id(azure_id)
        .await?
        .unwrap();
    assert_eq!(retrieved.verified_id_status, "onboarding_complete");
    println!("✅ Transition: verified -> onboarding_complete");

    println!("✅ E2E Test: Verified ID Status Transitions - PASSED");

    Ok(())
}

/// Test complete onboarding flow end-to-end (profile-focused)
#[tokio::test]
async fn test_complete_onboarding_flow() -> Result<()> {
    println!("🧪 E2E Test: Complete Onboarding Flow");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;

    // Step 1: Simulate post-auth profile creation
    let azure_id = "azure-complete-flow-789";
    let email = "complete@example.com";

    let mut profile = create_test_profile(azure_id, email, "Complete Flow User");

    client.store_user_profile(&profile).await?;
    println!("✅ Step 1: Initial profile created (post-auth)");

    // Step 2: Simulate personal info submission
    profile.given_name = Some("Complete".to_string());
    profile.family_name = Some("Flow".to_string());
    profile.display_name = "Complete Flow".to_string();
    profile.mobile_phone = Some("+1234567890".to_string());
    profile.updated_at = chrono::Utc::now().to_rfc3339();

    client.store_user_profile(&profile).await?;
    println!("✅ Step 2: Personal info updated");

    // Step 3: Simulate Verified ID issuance
    profile.verified_id_credential_id = Some("vc-credential-123".to_string());
    profile.verified_id_status = "verified".to_string();
    profile.verified_id_issued_at = Some(chrono::Utc::now().to_rfc3339());
    profile.updated_at = chrono::Utc::now().to_rfc3339();

    client.store_user_profile(&profile).await?;
    println!("✅ Step 3: Verified ID issued");

    // Step 4: Mark onboarding complete (Plaid linking would happen here in real flow)
    profile.verified_id_status = "onboarding_complete".to_string();
    profile.updated_at = chrono::Utc::now().to_rfc3339();

    client.store_user_profile(&profile).await?;
    println!("✅ Step 4: Onboarding marked complete");

    // Verify final state
    let final_profile = client
        .get_user_profile_by_azure_id(azure_id)
        .await?
        .unwrap();
    assert_eq!(final_profile.verified_id_status, "onboarding_complete");
    assert!(final_profile.verified_id_credential_id.is_some());
    assert!(final_profile.given_name.is_some());
    assert_eq!(final_profile.display_name, "Complete Flow");

    println!("✅ Final state verification passed");
    println!("✅ E2E Test: Complete Onboarding Flow - PASSED");

    Ok(())
}
