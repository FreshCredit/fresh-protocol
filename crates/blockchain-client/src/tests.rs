use std::collections::HashMap;

use super::*;

// Mutex to serialize tests that manipulate environment variables
static TEST_ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
use crate::helpers::{
    extract_report_id_from_key, generate_report_id, parse_h256_hex, user_id_to_account_id,
};

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
// ===========================================================================
// Account ID Generation Tests (User ID → Substrate AccountId32 mapping)
// ===========================================================================

#[test]
fn test_user_id_to_account_id_deterministic() {
    let account1 = user_id_to_account_id("user-123");
    let account2 = user_id_to_account_id("user-123");
    assert_eq!(account1, account2);
}

#[test]
fn test_user_id_to_account_id_different_users() {
    let account1 = user_id_to_account_id("user-123");
    let account2 = user_id_to_account_id("user-456");
    assert_ne!(account1, account2);
}

#[test]
fn test_user_id_to_account_id_empty_string() {
    let account = user_id_to_account_id("");
    // Should still produce valid 32-byte AccountId
    assert_eq!(account.0.len(), 32);
}

#[test]
fn test_user_id_to_account_id_long_string() {
    let long_id = "a".repeat(1000);
    let account = user_id_to_account_id(&long_id);
    assert_eq!(account.0.len(), 32);
}

// ===========================================================================
// Report Type Parsing Tests
// ===========================================================================

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_report_type_parse() {
    assert_eq!(
        ReportType::parse("plaid_financial"),
        ReportType::PlaidFinancial
    );
    assert_eq!(
        ReportType::parse("PlaidFinancial"),
        ReportType::PlaidFinancial
    );
    assert_eq!(
        ReportType::parse("identity_verification"),
        ReportType::IdentityVerification
    );
    assert_eq!(ReportType::parse("unknown"), ReportType::PlaidFinancial);
}

#[test]
fn test_report_type_all_variants() {
    assert_eq!(
        ReportType::parse("plaid_financial"),
        ReportType::PlaidFinancial
    );
    assert_eq!(
        ReportType::parse("identity_verification"),
        ReportType::IdentityVerification
    );
    assert_eq!(
        ReportType::parse("transaction_history"),
        ReportType::TransactionHistory
    );
    assert_eq!(
        ReportType::parse("balance_snapshot"),
        ReportType::BalanceSnapshot
    );
    assert_eq!(
        ReportType::parse("consumer_preferences"),
        ReportType::ConsumerPreferences
    );
    assert_eq!(
        ReportType::parse("provider_requirements"),
        ReportType::ProviderRequirements
    );
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_report_type_case_insensitive() {
    assert_eq!(
        ReportType::parse("PLAID_FINANCIAL"),
        ReportType::PlaidFinancial
    );
    assert_eq!(
        ReportType::parse("Plaid_Financial"),
        ReportType::PlaidFinancial
    );
}

#[test]
fn test_report_type_display() {
    assert_eq!(ReportType::PlaidFinancial.to_string(), "plaid_financial");
    assert_eq!(
        ReportType::IdentityVerification.to_string(),
        "identity_verification"
    );
    assert_eq!(
        ReportType::TransactionHistory.to_string(),
        "transaction_history"
    );
    assert_eq!(ReportType::BalanceSnapshot.to_string(), "balance_snapshot");
    assert_eq!(
        ReportType::ConsumerPreferences.to_string(),
        "consumer_preferences"
    );
    assert_eq!(
        ReportType::ProviderRequirements.to_string(),
        "provider_requirements"
    );
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_report_type_default() {
    let rt: ReportType = Default::default();
    assert_eq!(rt, ReportType::PlaidFinancial);
}

#[test]
fn test_report_type_as_runtime_type() {
    use freshcredit_runtime::runtime_types::pallet_freshcredit::ReportType as RuntimeReportType;
    assert!(matches!(
        ReportType::PlaidFinancial.as_runtime_type(),
        RuntimeReportType::PlaidFinancial
    ));
    assert!(matches!(
        ReportType::IdentityVerification.as_runtime_type(),
        RuntimeReportType::IdentityVerification
    ));
    assert!(matches!(
        ReportType::TransactionHistory.as_runtime_type(),
        RuntimeReportType::TransactionHistory
    ));
    assert!(matches!(
        ReportType::BalanceSnapshot.as_runtime_type(),
        RuntimeReportType::BalanceSnapshot
    ));
    assert!(matches!(
        ReportType::ConsumerPreferences.as_runtime_type(),
        RuntimeReportType::ConsumerPreferences
    ));
    assert!(matches!(
        ReportType::ProviderRequirements.as_runtime_type(),
        RuntimeReportType::ProviderRequirements
    ));
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_sanitize_user_id_for_logs() {
    assert_eq!(
        crate::client::BlockchainClient::sanitize_user_id_for_logs("user-12345678"),
        "user...5678"
    );
    assert_eq!(
        crate::client::BlockchainClient::sanitize_user_id_for_logs("short"),
        "***"
    );
    assert_eq!(
        crate::client::BlockchainClient::sanitize_user_id_for_logs("exactly8"),
        "***"
    );
    assert_eq!(
        crate::client::BlockchainClient::sanitize_user_id_for_logs("long-user-id-12345"),
        "long...2345"
    );
}

// ===========================================================================
// Report ID Generation Tests
// ===========================================================================

#[test]
fn test_generate_report_id_deterministic() {
    let id1 = generate_report_id("user-1", "plaid_financial", "abc123");
    let id2 = generate_report_id("user-1", "plaid_financial", "abc123");
    assert_eq!(id1, id2);
}

#[test]
fn test_generate_report_id_different_users() {
    let id1 = generate_report_id("user-1", "plaid_financial", "abc123");
    let id2 = generate_report_id("user-2", "plaid_financial", "abc123");
    assert_ne!(id1, id2);
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_generate_report_id_different_types() {
    let id1 = generate_report_id("user-1", "plaid_financial", "abc123");
    let id2 = generate_report_id("user-1", "credit_report", "abc123");
    assert_ne!(id1, id2);
}

#[test]
fn test_generate_report_id_different_hashes() {
    let id1 = generate_report_id("user-1", "plaid_financial", "hash1");
    let id2 = generate_report_id("user-1", "plaid_financial", "hash2");
    assert_ne!(id1, id2);
}

// ===========================================================================
// Request Serialization Tests
// ===========================================================================

#[test]
fn test_create_hash_request_serialization() {
    let mut report_data = HashMap::new();
    report_data.insert(
        "test".to_string(),
        serde_json::Value::String("value".to_string()),
    );

    let request = CreateHashRequest {
        user_id: "test-user".to_string(),
        report_type: "plaid_financial".to_string(),
        report_data: serde_json::to_value(&report_data).unwrap(),
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("test-user"));
    assert!(json.contains("plaid_financial"));
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_create_hash_request_with_complex_data() {
    let report_data = serde_json::json!({
        "accounts": [
            {"id": "acc1", "balance": 1000.50},
            {"id": "acc2", "balance": 2500.00}
        ],
        "metadata": {
            "source": "plaid",
            "timestamp": "2025-12-28T00:00:00Z"
        }
    });

    let request = CreateHashRequest {
        user_id: "complex-user".to_string(),
        report_type: "plaid_financial".to_string(),
        report_data,
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("complex-user"));
    assert!(json.contains("accounts"));
    assert!(json.contains("1000.5"));
}

#[test]
fn test_create_hash_response_deserialization() {
    let json = r#"{
        "id": "report-001",
        "user_id": "user-123",
        "report_type": "plaid_financial",
        "hash": "0xabc123def456",
        "block_number": 12345,
        "timestamp": "2025-12-28T00:00:00Z"
    }"#;

    let response: CreateHashResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.hash, "0xabc123def456");
    assert_eq!(response.block_number, 12345);
    assert_eq!(response.user_id, "user-123");
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_get_hash_response_deserialization() {
    let json = r#"{
        "id": "report-001",
        "user_id": "user-123",
        "report_type": "plaid_financial",
        "hash": "0xabc123def456",
        "block_number": 12345,
        "timestamp": "2025-12-28T00:00:00Z",
        "report_data": {"test": "value"}
    }"#;

    let response: GetHashResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.hash, "0xabc123def456");
    assert_eq!(response.id, "report-001");
    assert_eq!(response.report_type, "plaid_financial");
}

#[test]
fn test_verify_hash_response_verified() {
    let json = r#"{
        "verified": true,
        "hash": "0xabc123",
        "block_number": 12345,
        "timestamp": "2025-12-28T00:00:00Z",
        "user_id": "user-123",
        "report_type": "plaid_financial"
    }"#;

    let response: VerifyHashResponse = serde_json::from_str(json).unwrap();
    assert!(response.verified);
    assert_eq!(response.block_number, Some(12345));
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_verify_hash_response_not_verified() {
    let json = r#"{
        "verified": false,
        "hash": "0xabc123",
        "block_number": null,
        "timestamp": null,
        "user_id": null,
        "report_type": null
    }"#;

    let response: VerifyHashResponse = serde_json::from_str(json).unwrap();
    assert!(!response.verified);
    assert!(response.block_number.is_none());
}

// ===========================================================================
// BlockchainTransaction Serialization Tests
// ===========================================================================

#[test]
fn test_blockchain_transaction_serialization() {
    let tx = BlockchainTransaction {
        hash: "0xabc123".to_string(),
        amount: 1_000_000,
        from: "0xfrom".to_string(),
        to: "0xto".to_string(),
        block_number: 42,
        extrinsic_index: 7,
    };

    let json = serde_json::to_string(&tx).unwrap();
    assert!(json.contains("0xabc123"));
    assert!(json.contains("1000000"));
    assert!(json.contains("0xfrom"));
    assert!(json.contains("0xto"));
    assert!(json.contains("42"));
    assert!(json.contains('7'));
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_blockchain_transaction_deserialization() {
    let json = r#"{
        "hash": "0xdeadbeef",
        "amount": 5000,
        "from": "0xsender",
        "to": "0xreceiver",
        "block_number": 100,
        "extrinsic_index": 3
    }"#;

    let tx: BlockchainTransaction = serde_json::from_str(json).unwrap();
    assert_eq!(tx.hash, "0xdeadbeef");
    assert_eq!(tx.amount, 5000);
    assert_eq!(tx.from, "0xsender");
    assert_eq!(tx.to, "0xreceiver");
    assert_eq!(tx.block_number, 100);
    assert_eq!(tx.extrinsic_index, 3);
}

#[test]
fn test_blockchain_transaction_roundtrip() {
    let original = BlockchainTransaction {
        hash: "0xroundtrip".to_string(),
        amount: 99,
        from: "0xa".to_string(),
        to: "0xb".to_string(),
        block_number: 1,
        extrinsic_index: 0,
    };

    let json = serde_json::to_string(&original).unwrap();
    let restored: BlockchainTransaction = serde_json::from_str(&json).unwrap();

    assert_eq!(original.hash, restored.hash);
    assert_eq!(original.amount, restored.amount);
    assert_eq!(original.from, restored.from);
    assert_eq!(original.to, restored.to);
    assert_eq!(original.block_number, restored.block_number);
    assert_eq!(original.extrinsic_index, restored.extrinsic_index);
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
// ===========================================================================
// Hex Parsing Helper Tests
// ===========================================================================

#[test]
fn test_parse_h256_hex_valid_with_prefix() {
    let hex = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let result = parse_h256_hex(hex);
    assert!(result.is_some());
    let bytes = result.unwrap();
    assert_eq!(bytes.len(), 32);
    assert_eq!(bytes[0], 0x01);
    assert_eq!(bytes[31], 0xef);
}

#[test]
fn test_parse_h256_hex_valid_without_prefix() {
    let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let result = parse_h256_hex(hex);
    assert!(result.is_some());
}

#[test]
fn test_parse_h256_hex_invalid_hex_chars() {
    let hex = "0xzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
    assert!(parse_h256_hex(hex).is_none());
}

#[test]
fn test_parse_h256_hex_too_short() {
    let hex = "0x0123456789abcdef";
    assert!(parse_h256_hex(hex).is_none());
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_parse_h256_hex_too_long() {
    let hex = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef00";
    assert!(parse_h256_hex(hex).is_none());
}

#[test]
fn test_parse_h256_hex_empty() {
    assert!(parse_h256_hex("").is_none());
    assert!(parse_h256_hex("0x").is_none());
}

#[test]
fn test_parse_h256_hex_mixed_case() {
    let hex = "0xAbCdEf0123456789AbCdEf0123456789AbCdEf0123456789AbCdEf0123456789";
    assert!(parse_h256_hex(hex).is_some());
}

// ===========================================================================
// Report ID Extraction from Key Tests
// ===========================================================================

#[test]
fn test_extract_report_id_from_key_normal() {
    let prefix = [1u8, 2, 3, 4];
    let report_id = [5u8; 32];
    let mut key = Vec::new();
    key.extend_from_slice(&prefix);
    key.extend_from_slice(&report_id);

    let extracted = extract_report_id_from_key(&key);
    assert_eq!(extracted, report_id);
}

#[test]
fn test_extract_report_id_from_key_exact_32() {
    let key = [9u8; 32];
    let extracted = extract_report_id_from_key(&key);
    assert_eq!(extracted, key);
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_extract_report_id_from_key_long() {
    let mut key = vec![0u8; 100];
    key[68..100].copy_from_slice(&[7u8; 32]);
    let extracted = extract_report_id_from_key(&key);
    assert_eq!(extracted, [7u8; 32]);
}

#[test]
fn test_extract_report_id_from_key_short() {
    let key = [1u8, 2, 3];
    let extracted = extract_report_id_from_key(&key);
    assert_eq!(extracted, [0u8; 32]);
}

#[test]
fn test_extract_report_id_from_key_empty() {
    let extracted = extract_report_id_from_key(&[]);
    assert_eq!(extracted, [0u8; 32]);
}

// ===========================================================================
// Signer Loading Tests
// ===========================================================================

fn clear_signer_env() {
    for key in [
        "BLOCKCHAIN_SIGNER_MNEMONIC",
        "BLOCKCHAIN_SIGNER_SURI",
        "FRESHCREDIT_ENV",
        "K_SERVICE",
        "KUBERNETES_SERVICE_HOST",
        "PORT",
    ] {
        std::env::remove_var(key);
    }
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_load_signer_dev_fallback() {
    let _guard = TEST_ENV_MUTEX
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    clear_signer_env();
    // Force local mode to avoid races with parallel tests that set prod env vars
    std::env::set_var("FRESHCREDIT_ENV", "local");
    std::env::remove_var("K_SERVICE");
    std::env::remove_var("KUBERNETES_SERVICE_HOST");
    std::env::remove_var("PORT");

    let signer = BlockchainClient::load_signer();
    assert!(signer.is_ok());

    std::env::remove_var("FRESHCREDIT_ENV");
}

#[test]
fn test_load_signer_from_suri() {
    let _guard = TEST_ENV_MUTEX
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    clear_signer_env();
    std::env::set_var("BLOCKCHAIN_SIGNER_SURI", "//Bob");

    let signer = BlockchainClient::load_signer();
    assert!(signer.is_ok());

    std::env::remove_var("BLOCKCHAIN_SIGNER_SURI");
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_load_signer_from_mnemonic() {
    let _guard = TEST_ENV_MUTEX
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    clear_signer_env();
    // A valid BIP39 mnemonic
    std::env::set_var(
        "BLOCKCHAIN_SIGNER_MNEMONIC",
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    );

    let signer = BlockchainClient::load_signer();
    assert!(signer.is_ok());

    std::env::remove_var("BLOCKCHAIN_SIGNER_MNEMONIC");
}

#[test]
fn test_load_signer_production_missing_config() {
    let _guard = TEST_ENV_MUTEX
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    clear_signer_env();
    std::env::set_var("FRESHCREDIT_ENV", "production");

    let result = BlockchainClient::load_signer();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Missing required signer configuration"));

    std::env::remove_var("FRESHCREDIT_ENV");
}

// ===========================================================================
// Response Serialization Round-trip Tests
// ===========================================================================

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[test]
fn test_create_hash_response_roundtrip() {
    let original = CreateHashResponse {
        id: "report-001".to_string(),
        user_id: "user-123".to_string(),
        report_type: "plaid_financial".to_string(),
        hash: "0xabc123".to_string(),
        block_number: 12345,
        timestamp: "2025-12-28T00:00:00Z".to_string(),
    };

    let json = serde_json::to_string(&original).unwrap();
    let restored: CreateHashResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(original.id, restored.id);
    assert_eq!(original.hash, restored.hash);
    assert_eq!(original.block_number, restored.block_number);
}

#[test]
fn test_get_hash_response_roundtrip() {
    let original = GetHashResponse {
        id: "report-002".to_string(),
        user_id: "user-456".to_string(),
        report_type: "identity_verification".to_string(),
        hash: "0xdef456".to_string(),
        block_number: 999,
        timestamp: "2025-01-01T00:00:00Z".to_string(),
        report_data: serde_json::json!({"key": "value"}),
    };

    let json = serde_json::to_string(&original).unwrap();
    let restored: GetHashResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(original.id, restored.id);
    assert_eq!(original.report_data, restored.report_data);
}

#[test]
fn test_verify_hash_response_roundtrip() {
    let original = VerifyHashResponse {
        verified: true,
        hash: "0xverify".to_string(),
        block_number: Some(42),
        timestamp: Some("2025-06-15T12:00:00Z".to_string()),
        user_id: Some("user-789".to_string()),
        report_type: Some("plaid_financial".to_string()),
    };

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    let json = serde_json::to_string(&original).unwrap();
    let restored: VerifyHashResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(original.verified, restored.verified);
    assert_eq!(original.block_number, restored.block_number);
    assert_eq!(original.report_type, restored.report_type);
}

#[test]
fn test_verify_hash_response_none_fields_roundtrip() {
    let original = VerifyHashResponse {
        verified: false,
        hash: "0xnotfound".to_string(),
        block_number: None,
        timestamp: None,
        user_id: None,
        report_type: None,
    };

    let json = serde_json::to_string(&original).unwrap();
    let restored: VerifyHashResponse = serde_json::from_str(&json).unwrap();
    assert!(!restored.verified);
    assert!(restored.block_number.is_none());
    assert!(restored.timestamp.is_none());
}
