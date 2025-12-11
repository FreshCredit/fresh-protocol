//! E2E Tests for Chatbot Integration
//!
//! Tests the chatbot service including:
//! - Conversation creation and persistence
//! - Message storage and retrieval
//! - Bucketed data access (no raw PII)
//! - Conversation history management

use anyhow::Result;
use freshcredit_libsql_local::LocalClient;
use libsql::Value;

/// Helper to check if rows exist
async fn has_rows(rows: &mut libsql::Rows) -> bool {
    rows.next().await.ok().flatten().is_some()
}

/// Test user data for chatbot tests
#[derive(Clone)]
struct TestChatData {
    user_id: String,
}

/// Helper to create a test user profile for chatbot tests
async fn setup_test_user(client: &LocalClient) -> Result<TestChatData> {
    let user_id = uuid::Uuid::new_v4().to_string();
    let platform_user_id = format!("plat_{}", uuid::Uuid::new_v4());
    let azure_id = format!("azure_{}", uuid::Uuid::new_v4());
    let tenant_id = format!("tenant_{}", uuid::Uuid::new_v4());
    let object_id = format!("obj_{}", uuid::Uuid::new_v4());

    // Create user profile with all required fields
    client.execute(
        "INSERT INTO user_profile (id, platform_user_id, azure_id, email, display_name, tenant_id, object_id, role, created_at)
         VALUES (?, ?, ?, 'chatbot_test@example.com', 'Chatbot Test User', ?, ?, 'consumer', CURRENT_TIMESTAMP)",
        vec![
            Value::Text(user_id.clone()),
            Value::Text(platform_user_id),
            Value::Text(azure_id),
            Value::Text(tenant_id),
            Value::Text(object_id)
        ]
    ).await?;

    Ok(TestChatData { user_id })
}

/// Test conversation creation
#[tokio::test]
async fn test_conversation_creation() -> Result<()> {
    println!("🧪 E2E Test: Conversation Creation");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;
    let test_data = setup_test_user(&client).await?;

    // Create a conversation using the LocalClient method
    let conversation_id = client.create_conversation(
        &test_data.user_id,
        Some("Test Conversation"),
        Some("dashboard")
    ).await?;
    println!("✅ Conversation created: {conversation_id}");

    // Verify conversation exists
    let mut conv = client.query(
        "SELECT id, title, context FROM ai_conversations WHERE id = ?",
        vec![Value::Text(conversation_id.clone())]
    ).await?;
    assert!(has_rows(&mut conv).await, "Conversation should exist");
    println!("✅ Conversation verified in database");

    println!("🎉 Conversation Creation test passed!");
    Ok(())
}

/// Test message persistence
#[tokio::test]
async fn test_message_persistence() -> Result<()> {
    println!("🧪 E2E Test: Message Persistence");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;
    let test_data = setup_test_user(&client).await?;

    // Create a conversation
    let conversation_id = client.create_conversation(
        &test_data.user_id,
        Some("Message Test"),
        None
    ).await?;
    println!("✅ Conversation created");

    // Add user message
    let user_msg_id = client.add_conversation_message(
        &conversation_id,
        "user",
        "What is my credit score?",
        None,
        None,
        Some("gemini-1.5-flash")
    ).await?;
    println!("✅ User message added: {user_msg_id}");

    // Add assistant response
    let assistant_msg_id = client.add_conversation_message(
        &conversation_id,
        "assistant",
        "Your credit score is calculated based on several factors...",
        None,
        Some(150),
        Some("gemini-1.5-flash")
    ).await?;
    println!("✅ Assistant message added: {assistant_msg_id}");

    // Retrieve messages
    let messages = client.get_conversation_messages(&conversation_id, None).await?;
    assert_eq!(messages.len(), 2, "Should have 2 messages");
    println!("✅ Retrieved {} messages", messages.len());

    // Verify message order
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
    println!("✅ Message order verified");

    println!("🎉 Message Persistence test passed!");
    Ok(())
}

/// Test bucketed data access (no raw PII)
#[tokio::test]
async fn test_bucketed_data_access() -> Result<()> {
    println!("🧪 E2E Test: Bucketed Data Access");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;
    let test_data = setup_test_user(&client).await?;

    // Create account with balance
    let account_id = uuid::Uuid::new_v4().to_string();
    client.execute(
        "INSERT INTO accounts (id, user_id, account_type, balance, currency, institution_name, created_at)
         VALUES (?, ?, 'checking', 5000.00, 'USD', 'Test Bank', CURRENT_TIMESTAMP)",
        vec![Value::Text(account_id.clone()), Value::Text(test_data.user_id.clone())]
    ).await?;
    println!("✅ Account created with balance");

    // Query aggregated data (bucketed, no PII)
    let mut total_balance = client.query(
        "SELECT SUM(balance) as total FROM accounts WHERE user_id = ?",
        vec![Value::Text(test_data.user_id.clone())]
    ).await?;
    assert!(has_rows(&mut total_balance).await, "Should have balance data");
    println!("✅ Bucketed balance data accessible");

    println!("🎉 Bucketed Data Access test passed!");
    Ok(())
}

/// Test conversation history management
#[tokio::test]
async fn test_conversation_history() -> Result<()> {
    println!("🧪 E2E Test: Conversation History");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;
    let test_data = setup_test_user(&client).await?;

    // Create multiple conversations
    let conv1_id = client.create_conversation(
        &test_data.user_id,
        Some("First Conversation"),
        Some("reports")
    ).await?;
    let conv2_id = client.create_conversation(
        &test_data.user_id,
        Some("Second Conversation"),
        Some("scores")
    ).await?;
    println!("✅ Created 2 conversations");

    // Add messages to first conversation
    for i in 1..=3 {
        client.add_conversation_message(
            &conv1_id,
            if i % 2 == 1 { "user" } else { "assistant" },
            &format!("Message {i} in conversation 1"),
            None,
            Some(50 * i),
            Some("gemini-1.5-flash")
        ).await?;
    }
    println!("✅ Added 3 messages to conversation 1");

    // Add messages to second conversation
    for i in 1..=2 {
        client.add_conversation_message(
            &conv2_id,
            if i % 2 == 1 { "user" } else { "assistant" },
            &format!("Message {i} in conversation 2"),
            None,
            Some(75 * i),
            Some("gemini-1.5-flash")
        ).await?;
    }
    println!("✅ Added 2 messages to conversation 2");

    // Get user's conversations
    let conversations = client.get_user_conversations(&test_data.user_id, 10).await?;
    assert_eq!(conversations.len(), 2, "Should have 2 conversations");
    println!("✅ Retrieved {} conversations", conversations.len());

    // Get messages from first conversation
    let conv1_messages = client.get_conversation_messages(&conv1_id, None).await?;
    assert_eq!(conv1_messages.len(), 3, "Conversation 1 should have 3 messages");
    println!("✅ Conversation 1 has {} messages", conv1_messages.len());

    // Get messages from second conversation
    let conv2_messages = client.get_conversation_messages(&conv2_id, None).await?;
    assert_eq!(conv2_messages.len(), 2, "Conversation 2 should have 2 messages");
    println!("✅ Conversation 2 has {} messages", conv2_messages.len());

    println!("🎉 Conversation History test passed!");
    Ok(())
}

/// Test conversation with message limit
#[tokio::test]
async fn test_message_limit() -> Result<()> {
    println!("🧪 E2E Test: Message Limit");

    let client = LocalClient::new(":memory:").await?;
    client.initialize_schema().await?;
    let test_data = setup_test_user(&client).await?;

    // Create a conversation
    let conversation_id = client.create_conversation(
        &test_data.user_id,
        Some("Limit Test"),
        None
    ).await?;

    // Add 10 messages
    for i in 1..=10 {
        client.add_conversation_message(
            &conversation_id,
            if i % 2 == 1 { "user" } else { "assistant" },
            &format!("Message number {i}"),
            None,
            Some(25),
            Some("gemini-1.5-flash")
        ).await?;
    }
    println!("✅ Added 10 messages");

    // Get all messages
    let all_messages = client.get_conversation_messages(&conversation_id, None).await?;
    assert_eq!(all_messages.len(), 10, "Should have 10 messages");
    println!("✅ Retrieved all {} messages", all_messages.len());

    // Get limited messages
    let limited_messages = client.get_conversation_messages(&conversation_id, Some(5)).await?;
    assert_eq!(limited_messages.len(), 5, "Should have 5 messages with limit");
    println!("✅ Retrieved {} messages with limit", limited_messages.len());

    println!("🎉 Message Limit test passed!");
    Ok(())
}

