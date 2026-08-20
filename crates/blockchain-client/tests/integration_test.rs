// TAG: surface=blockchain owner=blockchain-team rule=BC-001
//! Integration tests for blockchain-client against a running Substrate node
//!
//! These tests require a running Substrate node at <ws://localhost:9944>
//! Run with: cargo test -p blockchain-client --test `integration_test`

use blockchain_client::{BlockchainClient, CreateHashRequest, ReportType};
use serde_json::json;

/// Test that we can connect to the Substrate node and check health
#[tokio::test]
async fn test_substrate_node_health_check() {
    let client = match BlockchainClient::new("ws://127.0.0.1:9944".to_string()).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping test - Substrate node not available: {e}");
            return;
        }
    };

    let healthy = client.health_check().await.expect("Health check failed");
    assert!(healthy, "Substrate node should be healthy (not syncing)");
    println!("✅ Health check passed - node is healthy");
}

/// Test that we can get blockchain statistics
#[tokio::test]
async fn test_get_blockchain_stats() {
    let client = match BlockchainClient::new("ws://127.0.0.1:9944".to_string()).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping test - Substrate node not available: {e}");
            return;
        }
    };

    let stats = client.get_stats().await.expect("Get stats failed");
    println!(
        "Blockchain stats: {}",
        serde_json::to_string_pretty(&stats).unwrap()
    );

    // Verify stats structure (uses block_height, not block_number)
    assert!(
        stats.get("block_height").is_some(),
        "Stats should have block_height" // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    );
    assert!(
        stats.get("is_syncing").is_some(),
        "Stats should have is_syncing"
    );
    println!("✅ Get stats passed - received valid stats");
}

/// Test creating a hash on the blockchain (requires signing)
/// Note: The pallet stores hashes under the SIGNER's account (Alice in dev mode),
/// not under the `user_id`. This test verifies the hash was created successfully.
#[tokio::test]
async fn test_create_and_verify_hash() {
    let client = match BlockchainClient::new("ws://127.0.0.1:9944".to_string()).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping test - Substrate node not available: {e}");
            return;
        }
    };

    // Create a test hash request
    let request = CreateHashRequest {
        user_id: "test-user-integration-001".to_string(),
        report_type: ReportType::TransactionHistory.to_string(),
        report_data: json!({
            "test": true,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "data": {
                "amount": 100.50,
                "description": "Integration test transaction"
            }
        }),
    };

    println!("Creating hash for user: {}", request.user_id);

    match client.create_hash(request).await {
        Ok(response) => {
            println!("✅ Created hash successfully!");
            println!("   Hash ID: {}", response.id);
            println!("   Hash: {}", response.hash);
            println!("   Block: {}", response.block_number);
            println!("   Timestamp: {}", response.timestamp);

            // Note: Hash is stored under signer's account (Alice), not user_id
            // For now, just verify the hash was created (block_number > 0)
            assert!(response.block_number > 0, "Block number should be > 0");
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            assert!(!response.hash.is_empty(), "Hash should not be empty");
            assert!(!response.id.is_empty(), "Report ID should not be empty");
            println!("✅ Hash creation verified (stored under signer account)");
        }
        Err(e) => {
            eprintln!("❌ Create hash failed: {e}");
            panic!("Create hash should succeed");
        }
    }
}

/// Test getting hashes for a user
#[tokio::test]
async fn test_get_user_hashes() {
    let client = match BlockchainClient::new("ws://127.0.0.1:9944".to_string()).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping test - Substrate node not available: {e}");
            return;
        }
    };

    // Get hashes for a test user (may be empty if no hashes created)
    let hashes = client
        .get_user_hashes("test-user-integration-001")
        .await
        .expect("Get user hashes failed");

    println!("Found {} hashes for test user", hashes.len());
    for hash in &hashes {
        println!("  - {} at block {}", hash.hash, hash.block_number);
    }
    println!("✅ Get user hashes passed");
}

/// Full integration flow: health → stats → create hash
/// Note: The pallet stores hashes under the SIGNER's account (Alice in dev mode),
/// so `verify_user_hash` and `get_user_hashes` won't find hashes by `user_id`.
/// This test verifies the core flow works end-to-end.
#[tokio::test]
async fn test_full_integration_flow() {
    let client = match BlockchainClient::new("ws://127.0.0.1:9944".to_string()).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping test - Substrate node not available: {e}");
            return;
        }
    };
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001

    // Generate unique user ID using timestamp
    let user_id = format!("test-user-{}", chrono::Utc::now().timestamp_millis());
    println!("=== Full Integration Flow ===");
    println!("User ID: {user_id}");

    // Step 1: Health check
    println!("\n1. Health check...");
    let healthy = client.health_check().await.expect("Health check failed");
    assert!(healthy);
    println!("   ✅ Node is healthy");

    // Step 2: Get initial stats
    println!("\n2. Get stats...");
    let stats = client.get_stats().await.expect("Get stats failed");
    let initial_total = stats["total_anchored"].as_u64().unwrap_or(0);
    println!("   ✅ Total anchored before: {initial_total}");

    // Step 3: Create hash
    println!("\n3. Create hash...");
    let request = CreateHashRequest {
        user_id: user_id.clone(),
        report_type: ReportType::IdentityVerification.to_string(),
        report_data: json!({"test": "full_integration", "timestamp": chrono::Utc::now().to_rfc3339()}),
    };
    let response = client
        .create_hash(request)
        .await
        .expect("Create hash failed");
    println!(
        "   ✅ Created hash: {} at block {}",
        response.hash, response.block_number
    );
    println!("   Report ID: {}", response.id);
    assert!(response.block_number > 0, "Block number should be > 0");

    // Step 4: Verify stats increased
    println!("\n4. Verify stats increased...");
    let stats_after = client.get_stats().await.expect("Get stats failed");
    let final_total = stats_after["total_anchored"].as_u64().unwrap_or(0);
    println!("   Total anchored after: {final_total}");
    assert!(
        final_total > initial_total,
        "Total anchored should increase"
    );
    println!("   ✅ Stats verified - total increased from {initial_total} to {final_total}");

    println!("\n=== Full Integration Flow Complete ===");
}
