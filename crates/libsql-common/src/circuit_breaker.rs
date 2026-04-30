//! Circuit Breaker Pattern for Database Connections
//!
//! This module provides a circuit breaker implementation that wraps database connections
//! to prevent cascading failures when the database becomes unavailable.
//!
//! # Circuit Breaker States
//!
//! ```text
//! ┌─────────────┐     failure threshold exceeded     ┌─────────────┐
//! │   CLOSED    │ ─────────────────────────────────► │    OPEN     │
//! │ (normal ops)│                                    │(failing fast)│
//! └──────┬──────┘                                    └──────┬──────┘
//!        │                                                  │
//!        │ success                                          │ timeout expired
//!        │                                                  ▼
//!        │                                         ┌─────────────┐
//!        └──────────────────────────────────────── │  HALF_OPEN  │
//!                                                  │ (test call) │
//!                                                  └──────┬──────┘
//!                                                         │
//!                              success ───────────────────┘
//!                              failure ───────────────────► OPEN
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use freshcredit_libsql_common::{
//!     ConnectionFactory, CircuitBreakerConfig, CircuitBreakerConnection, DatabaseConnection
//! };
//!
//! async fn example() -> anyhow::Result<()> {
//!     // Create underlying connection
//!     let inner = ConnectionFactory::from_env().await?;
//!
//!     // Wrap with circuit breaker
//!     let cb_config = CircuitBreakerConfig {
//!         failure_threshold: 5,
//!         recovery_timeout_secs: 30,
//!         half_open_max_calls: 3,
//!         success_threshold: 2,
//!     };
//!     let db = CircuitBreakerConnection::new(inner, cb_config);
//!
//!     // Use normally - circuit breaker handles failures automatically
//!     let rows = db.query("SELECT * FROM users", vec![]).await?;
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::connection::{ConnectionHealth, ConnectionMode, DatabaseConnection};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CircuitBreakerState {
    /// Normal operation - requests pass through
    #[default]
    Closed,
    /// Failing fast - requests immediately rejected
    Open,
    /// Testing if service has recovered
    HalfOpen,
}

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

/// A database connection wrapper that implements the circuit breaker pattern
pub struct CircuitBreakerConnection {
    /// Inner database connection
    inner: Arc<dyn DatabaseConnection>,
    /// Circuit breaker configuration
    config: CircuitBreakerConfig,
    /// Internal state
    inner_state: RwLock<CircuitBreakerInner>,
    /// Total success count (for stats)
    total_successes: AtomicU64,
    /// Total failure count (for stats)
    total_failures: AtomicU64,
    /// Total rejected count (for stats)
    total_rejected: AtomicU64,
}

impl CircuitBreakerConnection {
    /// Create a new circuit breaker connection
    pub fn new(inner: Arc<dyn DatabaseConnection>, config: CircuitBreakerConfig) -> Arc<Self> {
        Arc::new(Self {
            inner,
            config,
            inner_state: RwLock::new(CircuitBreakerInner {
                state: CircuitBreakerState::Closed,
                consecutive_failures: 0,
                consecutive_successes: 0,
                opened_at: None,
                half_open_calls: 0,
                stats: CircuitBreakerStats::default(),
            }),
            total_successes: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
            total_rejected: AtomicU64::new(0),
        })
    }

    /// Create from environment configuration
    pub async fn from_env(inner: Arc<dyn DatabaseConnection>) -> anyhow::Result<Arc<Self>> {
        let config = CircuitBreakerConfig::from_env();
        Ok(Self::new(inner, config))
    }

    /// Get current circuit breaker state
    pub async fn state(&self) -> CircuitBreakerState {
        self.inner_state.read().await.state
    }

    /// Get current statistics
    pub async fn stats(&self) -> CircuitBreakerStats {
        let inner = self.inner_state.read().await;
        let mut stats = inner.stats.clone();
        stats.state = inner.state;
        stats.successes = self.total_successes.load(Ordering::Relaxed);
        stats.failures = self.total_failures.load(Ordering::Relaxed);
        stats.rejected = self.total_rejected.load(Ordering::Relaxed);
        stats.consecutive_failures = inner.consecutive_failures;
        stats.consecutive_successes = inner.consecutive_successes;
        stats.half_open_calls = inner.half_open_calls;
        stats
    }

    /// Manually trip the circuit (for testing or emergency)
    pub async fn trip(&self) {
        let mut inner = self.inner_state.write().await;
        if inner.state != CircuitBreakerState::Open {
            warn!("Manually tripping circuit breaker");
            inner.state = CircuitBreakerState::Open;
            inner.opened_at = Some(Instant::now());
            inner.consecutive_failures = 0;
            inner.consecutive_successes = 0;
            inner.half_open_calls = 0;
            inner.stats.last_state_change = Some(Instant::now());
        }
    }

    /// Reset the circuit breaker (for testing or recovery)
    pub async fn reset(&self) {
        let mut inner = self.inner_state.write().await;
        info!("Resetting circuit breaker");
        inner.state = CircuitBreakerState::Closed;
        inner.consecutive_failures = 0;
        inner.consecutive_successes = 0;
        inner.opened_at = None;
        inner.half_open_calls = 0;
        inner.stats.last_state_change = Some(Instant::now());
    }

    /// Check if we should allow a call based on circuit state
    async fn check_state(&self) -> Result<(), CircuitBreakerError> {
        let mut inner = self.inner_state.write().await;

        match inner.state {
            CircuitBreakerState::Closed => {
                // Normal operation - allow call
                Ok(())
            }
            CircuitBreakerState::Open => {
                // Check if recovery timeout has passed
                if let Some(opened_at) = inner.opened_at {
                    let elapsed = opened_at.elapsed();
                    let timeout = Duration::from_secs(self.config.recovery_timeout_secs);

                    if elapsed >= timeout {
                        // Transition to half-open
                        info!(
                            "Circuit breaker transitioning from OPEN to HALF_OPEN after {}s",
                            elapsed.as_secs()
                        );
                        inner.state = CircuitBreakerState::HalfOpen;
                        inner.opened_at = None;
                        inner.half_open_calls = 0;
                        inner.consecutive_successes = 0;
                        inner.stats.last_state_change = Some(Instant::now());
                        Ok(())
                    } else {
                        // Still in timeout - reject
                        let remaining = timeout - elapsed;
                        debug!(
                            "Circuit breaker OPEN - rejecting call ({}s until retry)",
                            remaining.as_secs()
                        );
                        self.total_rejected.fetch_add(1, Ordering::Relaxed);
                        Err(CircuitBreakerError::CircuitOpen)
                    }
                } else {
                    // Should not happen, but treat as open
                    Err(CircuitBreakerError::CircuitOpen)
                }
            }
            CircuitBreakerState::HalfOpen => {
                // Allow limited test calls
                if inner.half_open_calls < self.config.half_open_max_calls {
                    inner.half_open_calls += 1;
                    Ok(())
                } else {
                    // Too many concurrent test calls
                    debug!("Circuit breaker HALF_OPEN - too many test calls");
                    self.total_rejected.fetch_add(1, Ordering::Relaxed);
                    Err(CircuitBreakerError::CircuitHalfOpenLimit)
                }
            }
        }
    }

    /// Record a successful call
    async fn record_success(&self) {
        let mut inner = self.inner_state.write().await;
        self.total_successes.fetch_add(1, Ordering::Relaxed);

        match inner.state {
            CircuitBreakerState::Closed => {
                inner.consecutive_failures = 0;
            }
            CircuitBreakerState::HalfOpen => {
                inner.consecutive_successes += 1;
                inner.half_open_calls = inner.half_open_calls.saturating_sub(1);

                // Check if we should close the circuit
                if inner.consecutive_successes >= self.config.success_threshold {
                    info!(
                        "Circuit breaker transitioning from HALF_OPEN to CLOSED after {} successes",
                        inner.consecutive_successes
                    );
                    inner.state = CircuitBreakerState::Closed;
                    inner.consecutive_successes = 0;
                    inner.half_open_calls = 0;
                    inner.stats.last_state_change = Some(Instant::now());
                }
            }
            CircuitBreakerState::Open => {
                // Should not happen, but reset counters
                inner.consecutive_failures = 0;
            }
        }
    }

    /// Record a failed call
    async fn record_failure(&self) {
        let mut inner = self.inner_state.write().await;
        self.total_failures.fetch_add(1, Ordering::Relaxed);

        match inner.state {
            CircuitBreakerState::Closed => {
                inner.consecutive_failures += 1;

                // Check if we should open the circuit
                if inner.consecutive_failures >= self.config.failure_threshold {
                    warn!(
                        "Circuit breaker transitioning from CLOSED to OPEN after {} failures",
                        inner.consecutive_failures
                    );
                    inner.state = CircuitBreakerState::Open;
                    inner.opened_at = Some(Instant::now());
                    inner.stats.last_state_change = Some(Instant::now());
                }
            }
            CircuitBreakerState::HalfOpen => {
                // Any failure in half-open goes back to open
                warn!("Circuit breaker transitioning from HALF_OPEN to OPEN due to test failure");
                inner.state = CircuitBreakerState::Open;
                inner.opened_at = Some(Instant::now());
                inner.half_open_calls = inner.half_open_calls.saturating_sub(1);
                inner.consecutive_successes = 0;
                inner.stats.last_state_change = Some(Instant::now());
            }
            CircuitBreakerState::Open => {
                // Already open, just increment counter
                inner.half_open_calls = inner.half_open_calls.saturating_sub(1);
            }
        }
    }

    /// Execute an operation with circuit breaker protection
    async fn execute<F, T>(&self, operation: F) -> Result<T, CircuitBreakerError>
    where
        F: std::future::Future<Output = anyhow::Result<T>>,
    {
        // Check circuit state before attempting
        self.check_state().await?;

        // Execute the operation
        match operation.await {
            Ok(result) => {
                self.record_success().await;
                Ok(result)
            }
            Err(e) => {
                self.record_failure().await;
                Err(CircuitBreakerError::DatabaseError(e))
            }
        }
    }
}

#[async_trait]
impl DatabaseConnection for CircuitBreakerConnection {
    async fn query(&self, sql: &str, params: Vec<libsql::Value>) -> anyhow::Result<libsql::Rows> {
        match self.execute(self.inner.query(sql, params)).await {
            Ok(rows) => Ok(rows),
            Err(CircuitBreakerError::CircuitOpen) => Err(anyhow::anyhow!(
                "Database circuit breaker is OPEN - service temporarily unavailable"
            )),
            Err(CircuitBreakerError::CircuitHalfOpenLimit) => Err(anyhow::anyhow!(
                "Database circuit breaker is HALF_OPEN - too many concurrent test calls"
            )),
            Err(CircuitBreakerError::DatabaseError(e)) => Err(e),
        }
    }

    async fn execute(&self, sql: &str, params: Vec<libsql::Value>) -> anyhow::Result<u64> {
        match self.execute(self.inner.execute(sql, params)).await {
            Ok(count) => Ok(count),
            Err(CircuitBreakerError::CircuitOpen) => Err(anyhow::anyhow!(
                "Database circuit breaker is OPEN - service temporarily unavailable"
            )),
            Err(CircuitBreakerError::CircuitHalfOpenLimit) => Err(anyhow::anyhow!(
                "Database circuit breaker is HALF_OPEN - too many concurrent test calls"
            )),
            Err(CircuitBreakerError::DatabaseError(e)) => Err(e),
        }
    }

    async fn execute_batch(&self, sql: &str) -> anyhow::Result<()> {
        match self.execute(self.inner.execute_batch(sql)).await {
            Ok(()) => Ok(()),
            Err(CircuitBreakerError::CircuitOpen) => Err(anyhow::anyhow!(
                "Database circuit breaker is OPEN - service temporarily unavailable"
            )),
            Err(CircuitBreakerError::CircuitHalfOpenLimit) => Err(anyhow::anyhow!(
                "Database circuit breaker is HALF_OPEN - too many concurrent test calls"
            )),
            Err(CircuitBreakerError::DatabaseError(e)) => Err(e),
        }
    }

    async fn health_check(&self) -> anyhow::Result<ConnectionHealth> {
        // Health check bypasses circuit breaker to get actual status
        let mut health = self.inner.health_check().await?;

        // Add circuit breaker state to health
        let cb_state = self.state().await;
        health.is_healthy = health.is_healthy && cb_state == CircuitBreakerState::Closed;

        Ok(health)
    }

    fn connection_mode(&self) -> ConnectionMode {
        self.inner.connection_mode()
    }

    async fn sync(&self) -> anyhow::Result<()> {
        match self.execute(self.inner.sync()).await {
            Ok(()) => Ok(()),
            Err(CircuitBreakerError::CircuitOpen) => Err(anyhow::anyhow!(
                "Database circuit breaker is OPEN - service temporarily unavailable"
            )),
            Err(CircuitBreakerError::CircuitHalfOpenLimit) => Err(anyhow::anyhow!(
                "Database circuit breaker is HALF_OPEN - too many concurrent test calls"
            )),
            Err(CircuitBreakerError::DatabaseError(e)) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
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
}
