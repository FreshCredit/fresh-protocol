//! Generic service circuit breaker.
//!
//! This module hosts the single, canonical implementation of the named
//! service circuit breaker used to protect external service calls. It was
//! consolidated from the duplicated stacks that previously lived in
//! `apps/adapters` and `crates/engine-ports`; those crates now re-export these
//! types. The database-oriented [`super::CircuitBreakerConnection`] remains the
//! canonical breaker for `DatabaseConnection` implementations.

use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use tracing::{info, warn};

pub use super::CircuitBreakerState as CircuitState;

/// Parse an environment variable, falling back to `default` (with a warning)
/// when the variable is set but unparsable.
fn parse_env<T: std::str::FromStr>(name: &str, default: T) -> T {
    match std::env::var(name) {
        Ok(v) => v.parse().unwrap_or_else(|_| {
            tracing::warn!("{} is set but invalid (\"{}\"), using default", name, v);
            default
        }),
        Err(_) => default,
    }
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Configuration for the service circuit breaker.
#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit (default: 5)
    pub failure_threshold: u32,
    /// Number of consecutive successes required to close the circuit (default: 3)
    pub success_threshold: u32,
    /// Duration to wait before attempting recovery (default: 60s)
    pub timeout_duration: Duration,
    /// Number of requests to allow in half-open state (default: 1)
    pub half_open_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl CircuitBreakerConfig {
    /// Create configuration with default values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout_duration: Duration::from_secs(60),
            half_open_requests: 1,
        }
    }

    /// Load configuration from environment variables with defaults.
    ///
    /// - `CIRCUIT_BREAKER_FAILURE_THRESHOLD` (default: 5)
    /// - `CIRCUIT_BREAKER_SUCCESS_THRESHOLD` (default: 3)
    /// - `CIRCUIT_BREAKER_TIMEOUT_SECONDS` (default: 60)
    /// - `CIRCUIT_BREAKER_HALF_OPEN_REQUESTS` (default: 1)
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            failure_threshold: parse_env("CIRCUIT_BREAKER_FAILURE_THRESHOLD", 5u32),
            success_threshold: parse_env("CIRCUIT_BREAKER_SUCCESS_THRESHOLD", 3u32),
            timeout_duration: Duration::from_secs(parse_env(
                "CIRCUIT_BREAKER_TIMEOUT_SECONDS",
                60u64,
            )),
            half_open_requests: parse_env("CIRCUIT_BREAKER_HALF_OPEN_REQUESTS", 1u32),
        }
    }

    /// Set the failure threshold.
    #[must_use]
    pub const fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Set the success threshold.
    #[must_use]
    pub const fn with_success_threshold(mut self, threshold: u32) -> Self {
        self.success_threshold = threshold;
        self
    }

    /// Set the timeout duration.
    #[must_use]
    pub const fn with_timeout_duration(mut self, duration: Duration) -> Self {
        self.timeout_duration = duration;
        self
    }

    /// Set the number of half-open requests.
    #[must_use]
    pub const fn with_half_open_requests(mut self, requests: u32) -> Self {
        self.half_open_requests = requests;
        self
    }
}

// TAG: surface=database owner=platform-team rule=DB-001
/// Errors that can occur when using the service circuit breaker.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CircuitBreakerError {
    /// The circuit is open and rejecting requests.
    #[error("Circuit breaker is OPEN for service '{service}', retry after {retry_after:?}")]
    CircuitOpen {
        /// Name of the protected service.
        service: String,
        /// How long to wait before retrying.
        retry_after: Duration,
    },
    /// The circuit is half-open but too many requests are in progress.
    #[error("Circuit breaker for '{service}' is busy testing recovery")]
    CircuitBusy {
        /// Name of the protected service.
        service: String,
    },
    /// The wrapped operation failed.
    #[error("Operation failed: {0}")]
    OperationFailed(String),
}

/// Metrics for circuit breaker monitoring.
#[derive(Debug, Default)]
pub struct CircuitBreakerMetrics {
    /// Total number of requests allowed through.
    pub requests_allowed: AtomicU64,
    /// Total number of requests rejected (circuit open).
    pub requests_rejected: AtomicU64,
    /// Total number of successful responses.
    pub responses_success: AtomicU64,
    /// Total number of failed responses.
    pub responses_failure: AtomicU64,
    /// Number of times circuit transitioned to open.
    pub circuit_opened: AtomicU64,
    /// Number of times circuit transitioned to closed.
    pub circuit_closed: AtomicU64,
    /// Number of times circuit entered half-open state.
    pub circuit_half_opened: AtomicU64,
}

impl CircuitBreakerMetrics {
    /// Create new circuit breaker metrics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn record_allowed(&self) {
        self.requests_allowed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_rejected(&self) {
        self.requests_rejected.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_success(&self) {
        self.responses_success.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_failure(&self) {
        self.responses_failure.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_opened(&self) {
        self.circuit_opened.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_closed(&self) {
        self.circuit_closed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_half_opened(&self) {
        self.circuit_half_opened.fetch_add(1, Ordering::Relaxed);
    }
}

/// Result of checking circuit state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitCheckResult {
    /// Request may proceed.
    Allow,
    /// Request may proceed as a half-open recovery test.
    AllowHalfOpen,
    /// Request must be rejected.
    Deny,
}

/// Internal mutable state of the circuit breaker.
#[derive(Debug)]
pub struct CircuitBreakerInner {
    /// Current state.
    pub(crate) state: CircuitState,
    /// Consecutive failure count.
    pub(crate) failure_count: u32,
    /// Consecutive success count (in half-open state).
    pub(crate) success_count: u32,
    /// Number of requests in progress (in half-open state).
    pub(crate) half_open_in_progress: u32,
    /// When the circuit was opened (for timeout calculation).
    pub(crate) opened_at: Option<Instant>,
    /// Last failure timestamp.
    pub(crate) last_failure_at: Option<Instant>,
}

impl CircuitBreakerInner {
    pub(crate) const fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            half_open_in_progress: 0,
            opened_at: None,
            last_failure_at: None,
        }
    }

    /// Check if the timeout has elapsed since the circuit was opened.
    #[must_use]
    pub fn is_timeout_elapsed(&self, timeout: Duration) -> bool {
        self.opened_at
            .is_some_and(|opened| opened.elapsed() >= timeout)
    }
}

/// Circuit breaker for protecting external service calls.
///
/// Thread-safe implementation using internal locking for state management.
/// Can be cloned and shared across multiple concurrent requests.
///
/// # Example
///
/// ```rust
/// use freshcredit_libsql_common::circuit_breaker::service::{
///     CircuitBreaker, CircuitBreakerConfig,
/// };
///
/// let config = CircuitBreakerConfig::new()
///     .with_failure_threshold(3)
///     .with_timeout_duration(std::time::Duration::from_secs(30));
///
/// let cb = CircuitBreaker::new("api-service", config);
///
/// // Clone for use in different contexts
/// let cb2 = cb.clone();
/// ```
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Name of the service being protected.
    pub(crate) name: String,
    /// Configuration.
    pub(crate) config: CircuitBreakerConfig,
    /// Internal mutable state.
    pub(crate) inner: Arc<RwLock<CircuitBreakerInner>>,
    /// Metrics for monitoring.
    pub(crate) metrics: Arc<CircuitBreakerMetrics>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    ///
    /// # Arguments
    /// * `name` - Name of the service (used for logging and metrics)
    /// * `config` - Circuit breaker configuration
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        let name = name.into();
        info!(
            service = %name,
            failure_threshold = config.failure_threshold,
            success_threshold = config.success_threshold,
            timeout_secs = config.timeout_duration.as_secs(),
            "Circuit breaker created"
        );

        Self {
            name,
            config,
            inner: Arc::new(RwLock::new(CircuitBreakerInner::new())),
            metrics: Arc::new(CircuitBreakerMetrics::new()),
        }
    }

    /// Get the service name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the current state.
    #[must_use]
    pub fn state(&self) -> CircuitState {
        self.inner
            .read()
            .map(|inner| inner.state)
            .unwrap_or(CircuitState::Open)
    }

    /// Get current metrics snapshot.
    #[must_use]
    pub fn metrics(&self) -> &CircuitBreakerMetrics {
        &self.metrics
    }

    /// Check if the circuit allows requests.
    ///
    /// Returns true if:
    /// - Circuit is Closed
    /// - Circuit is Open but timeout has elapsed (transitions to `HalfOpen`)
    /// - Circuit is `HalfOpen` and fewer than `half_open_requests` are in progress
    ///
    /// This is a non-blocking check that doesn't increment half-open counters.
    /// Use `call()` for full circuit breaker protection.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        match self.check_state() {
            CircuitCheckResult::Allow => {
                self.metrics.record_allowed();
                true
            }
            CircuitCheckResult::AllowHalfOpen => {
                // Don't record here - will be recorded in on_before_call
                true
            }
            CircuitCheckResult::Deny => {
                self.metrics.record_rejected();
                false
            }
        }
    }

    /// Execute a function with circuit breaker protection.
    ///
    /// # Errors
    ///
    /// Returns `Err(CircuitBreakerError)` if the circuit is open or the
    /// operation failed.
    pub async fn call<F, Fut, T, E>(&self, f: F) -> Result<T, CircuitBreakerError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        // Check if we can proceed
        let guard = self.on_before_call()?;

        // Execute the operation
        match f().await {
            Ok(result) => {
                guard.report_success();
                Ok(result)
            }
            Err(e) => {
                guard.report_failure();
                Err(CircuitBreakerError::OperationFailed(e.to_string()))
            }
        }
    }

    /// Execute a function with circuit breaker protection and custom error
    /// handling.
    ///
    /// Similar to `call`, but allows specifying which errors count as failures
    /// vs which should be passed through without affecting the circuit state.
    ///
    /// # Errors
    ///
    /// Returns `Err(CircuitBreakerError)` if the circuit is open or the
    /// operation failed.
    pub async fn call_with_error_filter<F, Fut, T, E, Filter>(
        &self,
        f: F,
        is_retriable: Filter,
    ) -> Result<T, CircuitBreakerError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
        Filter: FnOnce(&E) -> bool,
    {
        // Check if we can proceed
        let guard = self.on_before_call()?;

        // Execute the operation
        match f().await {
            Ok(result) => {
                guard.report_success();
                Ok(result)
            }
            Err(e) => {
                if is_retriable(&e) {
                    guard.report_failure();
                } else {
                    guard.report_non_retriable();
                }
                Err(CircuitBreakerError::OperationFailed(e.to_string()))
            }
        }
    }

    /// Record a successful request.
    ///
    /// For manual instrumentation; prefer `call()` which records automatically.
    pub fn record_success(&self) {
        self.on_success();
    }

    /// Record a failed request.
    ///
    /// For manual instrumentation; prefer `call()` which records automatically.
    pub fn record_failure(&self) {
        self.on_failure();
    }

    /// Get a prometheus-compatible metrics string.
    #[must_use]
    pub fn to_prometheus(&self) -> String {
        let metrics = self.metrics();
        format!(
            "# HELP circuit_breaker_state Current state (0=closed, 1=open, 2=half-open)\n\
             # TYPE circuit_breaker_state gauge\n\
             circuit_breaker{{service=\"{}\"}} {}\n\
             # HELP circuit_breaker_requests_allowed_total Total requests allowed\n\
             # TYPE circuit_breaker_requests_allowed_total counter\n\
             circuit_breaker_requests_allowed_total{{service=\"{}\"}} {}\n\
             # HELP circuit_breaker_requests_rejected_total Total requests rejected\n\
             # TYPE circuit_breaker_requests_rejected_total counter\n\
             circuit_breaker_requests_rejected_total{{service=\"{}\"}} {}\n\
             # HELP circuit_breaker_responses_success_total Total successful responses\n\
             # TYPE circuit_breaker_responses_success_total counter\n\
             circuit_breaker_responses_success_total{{service=\"{}\"}} {}\n\
             # HELP circuit_breaker_responses_failure_total Total failed responses\n\
             # TYPE circuit_breaker_responses_failure_total counter\n\
             circuit_breaker_responses_failure_total{{service=\"{}\"}} {}\n\
             # HELP circuit_breaker_state_changes_total Total state changes\n\
             # TYPE circuit_breaker_state_changes_total counter\n\
             circuit_breaker_opened_total{{service=\"{}\"}} {}\n\
             circuit_breaker_closed_total{{service=\"{}\"}} {}\n\
             circuit_breaker_half_opened_total{{service=\"{}\"}} {}\n",
            self.name,
            match self.state() {
                CircuitState::Closed => 0,
                CircuitState::Open => 1,
                CircuitState::HalfOpen => 2,
            },
            self.name,
            metrics.requests_allowed.load(Ordering::Relaxed),
            self.name,
            metrics.requests_rejected.load(Ordering::Relaxed),
            self.name,
            metrics.responses_success.load(Ordering::Relaxed),
            self.name,
            metrics.responses_failure.load(Ordering::Relaxed),
            self.name,
            metrics.circuit_opened.load(Ordering::Relaxed),
            self.name,
            metrics.circuit_closed.load(Ordering::Relaxed),
            self.name,
            metrics.circuit_half_opened.load(Ordering::Relaxed),
        )
    }

    // TAG: surface=database owner=platform-team rule=DB-001

    /// Called after a failed request.
    pub(crate) fn on_failure(&self) {
        let Ok(mut inner) = self.inner.write() else {
            return;
        };

        self.metrics.record_failure();
        inner.last_failure_at = Some(Instant::now());

        match inner.state {
            CircuitState::Closed => {
                inner.failure_count += 1;

                if inner.failure_count >= self.config.failure_threshold {
                    warn!(
                        service = %self.name,
                        failure_count = inner.failure_count,
                        threshold = self.config.failure_threshold,
                        "Circuit breaker transitioning from Closed to Open"
                    );
                    inner.state = CircuitState::Open;
                    inner.opened_at = Some(Instant::now());
                    drop(inner);
                    self.metrics.record_opened();
                }
            }

            CircuitState::HalfOpen => {
                inner.half_open_in_progress = inner.half_open_in_progress.saturating_sub(1);
                warn!(
                    service = %self.name,
                    "Circuit breaker transitioning from HalfOpen to Open (recovery test failed)"
                );
                inner.state = CircuitState::Open;
                inner.opened_at = Some(Instant::now());
                inner.success_count = 0;
                drop(inner);
                self.metrics.record_opened();
            }

            CircuitState::Open => {
                // Should not happen, but handle gracefully
                inner.half_open_in_progress = inner.half_open_in_progress.saturating_sub(1);
            }
        }
    }

    /// Check the current state without updating metrics.
    pub(crate) fn check_state(&self) -> CircuitCheckResult {
        let Ok(mut inner) = self.inner.write() else {
            return CircuitCheckResult::Deny; // Poisoned lock, fail safe
        };

        match inner.state {
            CircuitState::Closed => CircuitCheckResult::Allow,

            CircuitState::Open => {
                // Check if timeout has elapsed
                if inner.is_timeout_elapsed(self.config.timeout_duration) {
                    info!(
                        service = %self.name,
                        "Circuit breaker transitioning from Open to HalfOpen"
                    );
                    inner.state = CircuitState::HalfOpen;
                    inner.success_count = 0;
                    inner.half_open_in_progress = 0;
                    drop(inner);
                    self.metrics.record_half_opened();
                    CircuitCheckResult::AllowHalfOpen
                } else {
                    CircuitCheckResult::Deny
                }
            }

            CircuitState::HalfOpen => {
                // Allow limited requests in half-open state
                if inner.half_open_in_progress < self.config.half_open_requests {
                    CircuitCheckResult::AllowHalfOpen
                } else {
                    CircuitCheckResult::Deny
                }
            }
        }
    }

    /// Called before executing a request.
    ///
    /// Returns a [`CircuitCallGuard`] that must be consumed via
    /// `report_success`, `report_failure`, or `report_non_retriable`. If
    /// dropped without consumption, the guard automatically decrements
    /// `half_open_in_progress` to prevent cancellation leaks.
    ///
    /// # Errors
    ///
    /// Returns an error when the circuit is open or too many half-open tests
    /// are in progress.
    pub(crate) fn on_before_call(&self) -> Result<CircuitCallGuard, CircuitBreakerError> {
        let mut inner = self
            .inner
            .write()
            .map_err(|_| CircuitBreakerError::OperationFailed("Lock poisoned".to_string()))?;

        match inner.state {
            CircuitState::Closed => {
                self.metrics.record_allowed();
                Ok(CircuitCallGuard::new(self.clone()))
            }

            CircuitState::Open => {
                // Should not happen if check_state was called, but handle gracefully
                drop(inner);
                self.metrics.record_rejected();
                Err(CircuitBreakerError::CircuitOpen {
                    service: self.name.clone(),
                    retry_after: self.config.timeout_duration,
                })
            }

            CircuitState::HalfOpen => {
                if inner.half_open_in_progress < self.config.half_open_requests {
                    inner.half_open_in_progress += 1;
                    self.metrics.record_allowed();
                    Ok(CircuitCallGuard::new(self.clone()))
                } else {
                    drop(inner);
                    self.metrics.record_rejected();
                    Err(CircuitBreakerError::CircuitBusy {
                        service: self.name.clone(),
                    })
                }
            }
        }
    }

    /// Called after a successful request.
    pub(crate) fn on_success(&self) {
        let Ok(mut inner) = self.inner.write() else {
            return;
        };

        self.metrics.record_success();
        inner.last_failure_at = None;

        match inner.state {
            CircuitState::Closed => {
                // Reset failure count on success in closed state
                if inner.failure_count > 0 {
                    inner.failure_count = 0;
                }
            }

            CircuitState::HalfOpen => {
                inner.half_open_in_progress = inner.half_open_in_progress.saturating_sub(1);
                inner.success_count += 1;

                if inner.success_count >= self.config.success_threshold {
                    info!(
                        service = %self.name,
                        success_count = inner.success_count,
                        "Circuit breaker transitioning from HalfOpen to Closed"
                    );
                    inner.state = CircuitState::Closed;
                    inner.failure_count = 0;
                    inner.success_count = 0;
                    inner.opened_at = None;
                    drop(inner);
                    self.metrics.record_closed();
                }
            }

            CircuitState::Open => {
                // Should not happen, but handle gracefully
                inner.half_open_in_progress = inner.half_open_in_progress.saturating_sub(1);
            }
        }
    }
}

/// Guard that ensures `half_open_in_progress` is decremented on drop
/// if the call is cancelled before `on_success` or `on_failure` runs.
pub struct CircuitCallGuard {
    breaker: CircuitBreaker,
    consumed: bool,
}

impl CircuitCallGuard {
    const fn new(breaker: CircuitBreaker) -> Self {
        Self {
            breaker,
            consumed: false,
        }
    }

    /// Mark the guard as consumed and report success.
    pub fn report_success(mut self) {
        self.consumed = true;
        self.breaker.on_success();
    }

    /// Mark the guard as consumed and report failure.
    pub fn report_failure(mut self) {
        self.consumed = true;
        self.breaker.on_failure();
    }

    /// Report a non-retriable error (only decrements `half_open_in_progress`).
    pub fn report_non_retriable(mut self) {
        self.consumed = true;
        let Ok(mut inner) = self.breaker.inner.write() else {
            return;
        };
        if inner.state == CircuitState::HalfOpen {
            inner.half_open_in_progress = inner.half_open_in_progress.saturating_sub(1);
        }
    }
}

impl Drop for CircuitCallGuard {
    fn drop(&mut self) {
        if !self.consumed {
            // Call was cancelled - ensure half_open_in_progress is decremented
            let Ok(mut inner) = self.breaker.inner.write() else {
                return;
            };
            if inner.state == CircuitState::HalfOpen {
                inner.half_open_in_progress = inner.half_open_in_progress.saturating_sub(1);
            }
        }
    }
}

/// Builder for creating circuit breakers with common configurations.
#[derive(Debug)]
pub struct CircuitBreakerBuilder {
    name: String,
    config: CircuitBreakerConfig,
}

impl CircuitBreakerBuilder {
    /// Create a new builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            config: CircuitBreakerConfig::default(),
        }
    }

    /// Set failure threshold.
    #[must_use]
    pub const fn failure_threshold(mut self, threshold: u32) -> Self {
        self.config.failure_threshold = threshold;
        self
    }

    /// Set success threshold.
    #[must_use]
    pub const fn success_threshold(mut self, threshold: u32) -> Self {
        self.config.success_threshold = threshold;
        self
    }

    /// Set timeout duration.
    #[must_use]
    pub const fn timeout_duration(mut self, duration: Duration) -> Self {
        self.config.timeout_duration = duration;
        self
    }

    /// Set half-open request limit.
    #[must_use]
    pub const fn half_open_requests(mut self, requests: u32) -> Self {
        self.config.half_open_requests = requests;
        self
    }

    /// Build the circuit breaker.
    #[must_use]
    pub fn build(self) -> CircuitBreaker {
        CircuitBreaker::new(self.name, self.config)
    }

    /// Build and wrap in an Arc for shared use.
    #[must_use]
    pub fn build_arc(self) -> Arc<CircuitBreaker> {
        Arc::new(self.build())
    }
}

/// Create a circuit breaker optimized for payment gateways.
///
/// Uses shorter timeouts and lower thresholds for payment operations
/// where quick failure is preferred over waiting.
#[must_use]
pub fn payment_gateway_circuit(name: impl Into<String>) -> Arc<CircuitBreaker> {
    CircuitBreakerBuilder::new(name)
        .failure_threshold(3)
        .success_threshold(2)
        .timeout_duration(Duration::from_secs(30))
        .half_open_requests(1)
        .build_arc()
}

/// Create a circuit breaker optimized for discovery services.
///
/// Uses standard configuration suitable for read-only discovery operations.
#[must_use]
pub fn discovery_service_circuit(name: impl Into<String>) -> Arc<CircuitBreaker> {
    CircuitBreakerBuilder::new(name)
        .failure_threshold(5)
        .success_threshold(3)
        .timeout_duration(Duration::from_secs(60))
        .half_open_requests(1)
        .build_arc()
}

/// Create a circuit breaker optimized for third-party APIs.
///
/// Uses more tolerant settings for external APIs that may have occasional hiccups.
#[must_use]
pub fn third_party_api_circuit(name: impl Into<String>) -> Arc<CircuitBreaker> {
    CircuitBreakerBuilder::new(name)
        .failure_threshold(5)
        .success_threshold(3)
        .timeout_duration(Duration::from_secs(60))
        .half_open_requests(2)
        .build_arc()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering as AtomicOrdering;
    use tokio::time::sleep;

    #[test]
    fn test_circuit_state_display() {
        assert_eq!(CircuitState::Closed.to_string(), "closed");
        assert_eq!(CircuitState::Open.to_string(), "open");
        assert_eq!(CircuitState::HalfOpen.to_string(), "half-open");
    }

    #[test]
    fn test_config_default() {
        let config = CircuitBreakerConfig::new();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.success_threshold, 3);
        assert_eq!(config.timeout_duration, Duration::from_secs(60));
        assert_eq!(config.half_open_requests, 1);
    }

    #[test]
    fn test_config_builder() {
        let config = CircuitBreakerConfig::new()
            .with_failure_threshold(10)
            .with_success_threshold(5)
            .with_timeout_duration(Duration::from_secs(120))
            .with_half_open_requests(3);

        assert_eq!(config.failure_threshold, 10);
        assert_eq!(config.success_threshold, 5);
        assert_eq!(config.timeout_duration, Duration::from_secs(120));
        assert_eq!(config.half_open_requests, 3);
    }

    #[test]
    fn test_circuit_breaker_creation() {
        let cb = CircuitBreaker::new("test-service", CircuitBreakerConfig::new());
        assert_eq!(cb.name(), "test-service");
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_clone() {
        let cb1 = CircuitBreaker::new("test", CircuitBreakerConfig::new());
        let cb2 = cb1.clone();

        assert_eq!(cb1.name(), cb2.name());
        // Both should point to the same internal state
    }

    #[tokio::test]
    async fn test_call_success() {
        let cb = CircuitBreaker::new("test", CircuitBreakerConfig::new());

        let result = cb.call(|| async { Ok::<_, anyhow::Error>(42) }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_call_failure() {
        let cb = CircuitBreaker::new(
            "test",
            CircuitBreakerConfig::new().with_failure_threshold(1),
        );

        let result = cb
            .call(|| async { Err::<i32, anyhow::Error>(anyhow::anyhow!("test error")) })
            .await;

        assert!(result.is_err());
        // Circuit should be open after 1 failure
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_opens_after_threshold() {
        let cb = CircuitBreaker::new(
            "test",
            CircuitBreakerConfig::new().with_failure_threshold(3),
        );

        // 2 failures - circuit still closed
        for _ in 0..2 {
            let _ = cb
                .call(|| async { Err::<i32, anyhow::Error>(anyhow::anyhow!("error")) })
                .await;
        }
        assert_eq!(cb.state(), CircuitState::Closed);

        // 3rd failure - circuit opens
        let _ = cb
            .call(|| async { Err::<i32, anyhow::Error>(anyhow::anyhow!("error")) })
            .await;
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_rejects_when_open() {
        let cb = CircuitBreaker::new(
            "test",
            CircuitBreakerConfig::new()
                .with_failure_threshold(1)
                .with_timeout_duration(Duration::from_secs(3600)), // Long timeout
        );

        // Open the circuit
        let _ = cb
            .call(|| async { Err::<i32, anyhow::Error>(anyhow::anyhow!("error")) })
            .await;
        assert_eq!(cb.state(), CircuitState::Open);

        // Next call should be rejected immediately
        let result = cb.call(|| async { Ok::<_, anyhow::Error>(42) }).await;

        assert!(matches!(
            result,
            Err(CircuitBreakerError::CircuitOpen { .. })
        ));
    }

    #[tokio::test]
    async fn test_circuit_half_open_transition() {
        let cb = CircuitBreaker::new(
            "test",
            CircuitBreakerConfig::new()
                .with_failure_threshold(1)
                .with_timeout_duration(Duration::from_millis(10)),
        );

        // Open the circuit
        let _ = cb
            .call(|| async { Err::<i32, anyhow::Error>(anyhow::anyhow!("error")) })
            .await;
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for timeout
        sleep(Duration::from_millis(20)).await;

        // Circuit should transition to half-open
        assert!(cb.is_allowed());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn test_circuit_closes_after_success_threshold() {
        // P1 FIX: Use actual sleep instead of paused time
        // The circuit breaker uses std::time::Instant which doesn't respect tokio paused time
        let cb = CircuitBreaker::new(
            "test",
            CircuitBreakerConfig::new()
                .with_failure_threshold(1)
                .with_success_threshold(2)
                .with_timeout_duration(Duration::from_millis(10)),
        );

        // Open the circuit
        let _ = cb
            .call(|| async { Err::<i32, anyhow::Error>(anyhow::anyhow!("error")) })
            .await;
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for timeout to enter half-open
        sleep(Duration::from_millis(20)).await;

        // Trigger state check by calling is_allowed() which checks timeout
        assert!(cb.is_allowed()); // Should transition to HalfOpen and allow
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // First success in half-open
        let _ = cb.call(|| async { Ok::<_, anyhow::Error>(1) }).await;
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Second success should close the circuit (no need to wait in half-open)
        let _ = cb.call(|| async { Ok::<_, anyhow::Error>(2) }).await;
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_reopens_on_half_open_failure() {
        let cb = CircuitBreaker::new(
            "test",
            CircuitBreakerConfig::new()
                .with_failure_threshold(1)
                .with_timeout_duration(Duration::from_millis(1)),
        );

        // Open the circuit
        let _ = cb
            .call(|| async { Err::<i32, anyhow::Error>(anyhow::anyhow!("error")) })
            .await;
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for timeout
        sleep(Duration::from_millis(10)).await;

        // Failure in half-open should reopen
        let _ = cb
            .call(|| async { Err::<i32, anyhow::Error>(anyhow::anyhow!("error")) })
            .await;
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_call_with_error_filter() {
        let cb = CircuitBreaker::new(
            "test",
            CircuitBreakerConfig::new().with_failure_threshold(1),
        );

        // Client error (4xx) should not count as failure
        let result = cb
            .call_with_error_filter(
                || async { Err::<i32, anyhow::Error>(anyhow::anyhow!("client error")) },
                |e| !e.to_string().contains("client"),
            )
            .await;

        assert!(result.is_err());
        // Circuit should still be closed because error was filtered
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_metrics_tracking() {
        let cb = CircuitBreaker::new("test", CircuitBreakerConfig::new());
        let metrics = cb.metrics();

        metrics.record_allowed();
        metrics.record_allowed();
        metrics.record_rejected();

        assert_eq!(metrics.requests_allowed.load(AtomicOrdering::Relaxed), 2);
        assert_eq!(metrics.requests_rejected.load(AtomicOrdering::Relaxed), 1);
    }

    #[test]
    fn test_prometheus_output() {
        let cb = CircuitBreaker::new("test-service", CircuitBreakerConfig::new());
        let prom = cb.to_prometheus();

        assert!(prom.contains("circuit_breaker"));
        assert!(prom.contains("test-service"));
        assert!(prom.contains("circuit_breaker_requests_allowed_total"));
        assert!(prom.contains("circuit_breaker_requests_rejected_total"));
    }

    #[test]
    fn test_circuit_breaker_builder() {
        let cb = CircuitBreakerBuilder::new("test")
            .failure_threshold(10)
            .success_threshold(5)
            .timeout_duration(Duration::from_secs(120))
            .half_open_requests(3)
            .build();

        assert_eq!(cb.name(), "test");
        // Verify config was applied by checking behavior
    }

    #[test]
    fn test_preconfigured_circuits() {
        let payment = payment_gateway_circuit("stripe");
        assert_eq!(payment.name(), "stripe");

        let discovery = discovery_service_circuit("ucp");
        assert_eq!(discovery.name(), "ucp");

        let api = third_party_api_circuit("external");
        assert_eq!(api.name(), "external");
    }

    #[test]
    fn test_error_display() {
        let err = CircuitBreakerError::CircuitOpen {
            service: "test".to_string(),
            retry_after: Duration::from_secs(30),
        };
        assert!(err.to_string().contains("OPEN"));
        assert!(err.to_string().contains("test"));

        let err = CircuitBreakerError::CircuitBusy {
            service: "test".to_string(),
        };
        assert!(err.to_string().contains("busy"));

        let err = CircuitBreakerError::OperationFailed("something went wrong".to_string());
        assert!(err.to_string().contains("something went wrong"));
    }

    // =============================================================================
    // Cancellation safety tests (P0-SECURITY fix)
    // =============================================================================

    #[tokio::test]
    async fn test_guard_drop_decrements_half_open() {
        let cb = Arc::new(CircuitBreaker::new(
            "test",
            CircuitBreakerConfig::new()
                .with_failure_threshold(1)
                .with_timeout_duration(Duration::from_secs(0))
                .with_half_open_requests(1),
        ));

        // Open the circuit
        cb.call(|| async { Err::<(), String>("fail".to_string()) })
            .await
            .unwrap_err();
        assert_eq!(cb.state(), CircuitState::Open);

        // Transition to HalfOpen via check_state
        let _ = cb.check_state();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // on_before_call increments half_open_in_progress
        let guard = cb.on_before_call().unwrap();
        {
            let inner = cb.inner.read().unwrap();
            assert_eq!(inner.half_open_in_progress, 1);
            drop(inner);
        }

        // Drop guard without consuming it (simulates cancellation)
        drop(guard);

        // half_open_in_progress should be decremented
        {
            let inner = cb.inner.read().unwrap();
            assert_eq!(inner.half_open_in_progress, 0);
            drop(inner);
        }
    }

    #[test]
    fn test_guard_success_does_not_decrement_on_drop() {
        let cb = CircuitBreaker::new("test", CircuitBreakerConfig::new());

        let guard = cb.on_before_call().unwrap();
        guard.report_success();

        // State should be closed, success recorded
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_guard_failure_does_not_decrement_on_drop() {
        let cb = CircuitBreaker::new(
            "test",
            CircuitBreakerConfig::new().with_failure_threshold(10),
        );

        let guard = cb.on_before_call().unwrap();
        guard.report_failure();

        // Failure count should be incremented
        let inner = cb.inner.read().unwrap();
        assert_eq!(inner.failure_count, 1);
        drop(inner);
    }

    #[test]
    #[serial_test::serial]
    fn test_parse_env_with_valid_value() {
        std::env::set_var("TEST_PARSE_ENV_VALID_42", "42");
        let result: i32 = parse_env("TEST_PARSE_ENV_VALID_42", 0);
        assert_eq!(result, 42);
        std::env::remove_var("TEST_PARSE_ENV_VALID_42");
    }

    #[test]
    #[serial_test::serial]
    fn test_parse_env_with_invalid_value_uses_default() {
        std::env::set_var("TEST_PARSE_ENV_INVALID_X", "not_a_number");
        let result: i32 = parse_env("TEST_PARSE_ENV_INVALID_X", 99);
        assert_eq!(result, 99);
        std::env::remove_var("TEST_PARSE_ENV_INVALID_X");
    }

    #[test]
    fn test_parse_env_missing_uses_default() {
        std::env::remove_var("TEST_PARSE_ENV_MISSING_Y");
        let result: i32 = parse_env("TEST_PARSE_ENV_MISSING_Y", 77);
        assert_eq!(result, 77);
    }

    #[test]
    #[serial_test::serial]
    fn test_config_from_env() {
        std::env::set_var("CIRCUIT_BREAKER_FAILURE_THRESHOLD", "7");
        std::env::set_var("CIRCUIT_BREAKER_SUCCESS_THRESHOLD", "4");
        std::env::set_var("CIRCUIT_BREAKER_TIMEOUT_SECONDS", "45");
        std::env::set_var("CIRCUIT_BREAKER_HALF_OPEN_REQUESTS", "2");

        let config = CircuitBreakerConfig::from_env();
        assert_eq!(config.failure_threshold, 7);
        assert_eq!(config.success_threshold, 4);
        assert_eq!(config.timeout_duration, Duration::from_secs(45));
        assert_eq!(config.half_open_requests, 2);

        std::env::remove_var("CIRCUIT_BREAKER_FAILURE_THRESHOLD");
        std::env::remove_var("CIRCUIT_BREAKER_SUCCESS_THRESHOLD");
        std::env::remove_var("CIRCUIT_BREAKER_TIMEOUT_SECONDS");
        std::env::remove_var("CIRCUIT_BREAKER_HALF_OPEN_REQUESTS");
    }
}
