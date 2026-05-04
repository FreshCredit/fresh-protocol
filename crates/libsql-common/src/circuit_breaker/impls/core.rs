use super::helpers::map_cb_result;
use super::*;

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
        map_cb_result(self.execute(self.inner.query(sql, params)).await)
    }

    async fn execute(&self, sql: &str, params: Vec<libsql::Value>) -> anyhow::Result<u64> {
        map_cb_result(self.execute(self.inner.execute(sql, params)).await)
    }

    async fn execute_batch(&self, sql: &str) -> anyhow::Result<()> {
        map_cb_result(self.execute(self.inner.execute_batch(sql)).await)
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
        map_cb_result(self.execute(self.inner.sync()).await)
    }
}
