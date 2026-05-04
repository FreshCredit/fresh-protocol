use super::*;

#[tokio::test]
async fn test_create_in_memory() {
    let conn = ConnectionFactory::create_in_memory().await.unwrap();

    let health = conn.health_check().await.unwrap();
    assert!(health.is_healthy);
    assert_eq!(health.mode, ConnectionMode::LocalOnly);
}

#[tokio::test]
async fn test_create_local() {
    let config = ConnectionConfig {
        mode: ConnectionMode::LocalOnly,
        local_path: Some(std::path::PathBuf::from(":memory:")),
        ..Default::default()
    };

    let conn = ConnectionFactory::create_local(&config).await.unwrap();

    let health = conn.health_check().await.unwrap();
    assert!(health.is_healthy);
}

#[tokio::test]
async fn test_config_validation() {
    // Missing local path for replica mode
    let config = ConnectionConfig {
        mode: ConnectionMode::EmbeddedReplica,
        remote_url: "test".to_string(),
        local_path: None,
        ..Default::default()
    };

    let result = ConnectionFactory::create(&config).await;
    assert!(result.is_err());
}
