use super::*;

mod core;
mod helpers;
#[cfg(test)]
mod tests;

pub use core::CircuitBreakerConnection;
impl fmt::Display for CircuitBreakerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitBreakerState::Closed => write!(f, "closed"),
            CircuitBreakerState::Open => write!(f, "open"),
            CircuitBreakerState::HalfOpen => write!(f, "half-open"),
        }
    }
}

/// Configuration for circuit breaker
#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit
    pub failure_threshold: u32,
    /// Time to wait before attempting recovery (seconds)
    pub recovery_timeout_secs: u64,
    /// Maximum number of test calls in half-open state
    pub half_open_max_calls: u32,
    /// Success threshold to close circuit from half-open
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout_secs: 30,
            half_open_max_calls: 3,
            success_threshold: 2,
        }
    }
}

impl CircuitBreakerConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        use std::env;

        let failure_threshold = env::var("DB_CB_FAILURE_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let recovery_timeout_secs = env::var("DB_CB_RECOVERY_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let half_open_max_calls = env::var("DB_CB_HALF_OPEN_MAX_CALLS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);

        let success_threshold = env::var("DB_CB_SUCCESS_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);

        Self {
            failure_threshold,
            recovery_timeout_secs,
            half_open_max_calls,
            success_threshold,
        }
    }
}

/// Statistics for circuit breaker
#[derive(Debug, Clone, Default)]
pub struct CircuitBreakerStats {
    /// Current state
    pub state: CircuitBreakerState,
    /// Total successful calls
    pub successes: u64,
    /// Total failed calls
    pub failures: u64,
    /// Total rejected calls (circuit open)
    pub rejected: u64,
    /// Consecutive failures in current window
    pub consecutive_failures: u32,
    /// Consecutive successes in half-open state
    pub consecutive_successes: u32,
    /// Last state change timestamp
    pub last_state_change: Option<Instant>,
    /// Current test calls in half-open state
    pub half_open_calls: u32,
}

/// Internal state management for circuit breaker
struct CircuitBreakerInner {
    /// Current state
    state: CircuitBreakerState,
    /// Consecutive failure count
    consecutive_failures: u32,
    /// Consecutive success count (for half-open)
    consecutive_successes: u32,
    /// Time when circuit was opened
    opened_at: Option<Instant>,
    /// Test calls in half-open state
    half_open_calls: u32,
    /// Total statistics
    stats: CircuitBreakerStats,
}

/// Circuit breaker error types
#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError {
    #[error("Circuit breaker is OPEN - database unavailable")]
    CircuitOpen,
    #[error("Circuit breaker is HALF_OPEN - too many test calls")]
    CircuitHalfOpenLimit,
    #[error("Database operation failed: {0}")]
    DatabaseError(#[from] anyhow::Error),
}
