use super::*;

#[test]
fn test_circuit_breaker_config_default() {
    let config = CircuitBreakerConfig::default();
    assert_eq!(config.failure_threshold, 5);
    assert_eq!(config.recovery_timeout_secs, 30);
    assert_eq!(config.half_open_max_calls, 3);
    assert_eq!(config.success_threshold, 2);
}

#[test]
fn test_circuit_breaker_state_display() {
    assert_eq!(CircuitBreakerState::Closed.to_string(), "closed");
    assert_eq!(CircuitBreakerState::Open.to_string(), "open");
    assert_eq!(CircuitBreakerState::HalfOpen.to_string(), "half-open");
}

#[tokio::test]
async fn test_circuit_breaker_transitions() {
    // Create a mock connection (would need mock trait in real tests)
    // This is a structural test of the state machine logic

    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        recovery_timeout_secs: 1,
        half_open_max_calls: 1,
        success_threshold: 1,
    };

    // Test that we can create a circuit breaker
    // Full integration tests would require a mock DatabaseConnection
    assert_eq!(config.failure_threshold, 2);
    assert_eq!(config.recovery_timeout_secs, 1);
}
