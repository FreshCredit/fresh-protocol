use std::path::PathBuf;

use super::types::*;

#[test]
fn test_connection_mode_from_str() {
    assert_eq!(
        "remote".parse::<ConnectionMode>().unwrap(),
        ConnectionMode::DirectRemote
    );
    assert_eq!(
        "replica".parse::<ConnectionMode>().unwrap(),
        ConnectionMode::EmbeddedReplica
    );
    assert_eq!(
        "local".parse::<ConnectionMode>().unwrap(),
        ConnectionMode::LocalOnly
    );
    assert_eq!(
        "adaptive".parse::<ConnectionMode>().unwrap(),
        ConnectionMode::Adaptive
    );
    assert!("unknown".parse::<ConnectionMode>().is_err());
}

#[test]
fn test_read_consistency_from_str() {
    assert_eq!(
        "eventual".parse::<ReadConsistency>().unwrap(),
        ReadConsistency::Eventual
    );
    assert_eq!(
        "strong".parse::<ReadConsistency>().unwrap(),
        ReadConsistency::Strong
    );
    assert_eq!(
        "adaptive".parse::<ReadConsistency>().unwrap(),
        ReadConsistency::Adaptive
    );
}

#[test]
fn test_config_validate() {
    // Remote mode requires URL
    let config = ConnectionConfig {
        mode: ConnectionMode::DirectRemote,
        remote_url: String::new(),
        ..Default::default()
    };
    assert!(config.validate().is_err());

    // Replica mode requires URL and local path
    let config = ConnectionConfig {
        mode: ConnectionMode::EmbeddedReplica,
        remote_url: "test".to_string(),
        local_path: None,
        ..Default::default()
    };
    assert!(config.validate().is_err());

    // Local mode requires path
    let config = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: None,
        ..Default::default()
    };
    assert!(config.validate().is_err());

    // Valid config
    let config = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: Some(PathBuf::from("/tmp/test.db")),
        ..Default::default()
    };
    assert!(config.validate().is_ok());
}
