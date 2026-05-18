//! Database connection factory with retry logic
//!
//! Provides centralized connection creation with exponential backoff retry
//! for resilient database connections.

use rand::Rng;
use std::future::Future;
use std::time::Duration;
use tracing::{info, warn};

/// Configuration for database connection retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Jitter percentage (0.0 to 1.0) to prevent thundering herd
    pub jitter_percentage: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            // HARDCODED_TIMEOUT: Database connection retry delays
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(10),
            max_attempts: 3,
            jitter_percentage: 0.2, // ±20%
        }
    }
}

impl RetryConfig {
    /// Create from environment variables with fallback to defaults
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            initial_delay: Duration::from_millis(
                std::env::var("DB_RETRY_INITIAL_DELAY_MS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1000),
            ),
            max_delay: Duration::from_millis(
                std::env::var("DB_RETRY_MAX_DELAY_MS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(10000),
            ),
            max_attempts: std::env::var("DB_RETRY_MAX_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(3),
            jitter_percentage: 0.2,
        }
    }

    /// Calculate delay for a given attempt number
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap
    )]
    fn delay_for_attempt(&self, attempt: u32) -> Option<Duration> {
        if attempt >= self.max_attempts {
            return None;
        }

        // Exponential backoff: initial_delay * 2^attempt
        let base_delay = self.initial_delay.as_millis() as u64 * 2u64.pow(attempt);
        let capped_delay = base_delay.min(self.max_delay.as_millis() as u64);

        // Add jitter to prevent thundering herd
        let jitter_range = (capped_delay as f64 * self.jitter_percentage) as i64;
        let mut rng = rand::thread_rng();
        let jitter: i64 = rng.gen_range(-jitter_range..=jitter_range);
        let final_delay = (capped_delay as i64 + jitter).max(0) as u64;

        Some(Duration::from_millis(final_delay))
    }
}

/// Execute an async operation with retry logic
///
/// # Example
///
/// ```ignore
/// use freshcredit_libsql_common::{RetryConfig, with_retry};
///
/// let config = RetryConfig::default();
/// let result = with_retry(&config, || async {
///     CloudClient::new(&url, &token).await
/// }).await;
/// ```
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn with_retry<F, Fut, T, E>(config: &RetryConfig, mut operation: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 0;

    // SIM-LOOP-001: Bounded retry loop - exits on success or max attempts
    loop {
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    info!("Database connection succeeded after {} retries", attempt);
                }
                return Ok(result);
            }
            Err(err) => {
                if let Some(delay) = config.delay_for_attempt(attempt) {
                    warn!(
                        "Database connection attempt {} failed: {}. Retrying in {:?}",
                        attempt + 1,
                        err,
                        delay
                    );
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                } else {
                    warn!(
                        "Database connection failed after {} attempts: {}",
                        config.max_attempts, err
                    );
                    return Err(err);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(1000));
        assert_eq!(config.max_delay, Duration::from_secs(10));
    }

    #[test]
    fn test_delay_calculation() {
        let config = RetryConfig {
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(60),
            max_attempts: 5,
            jitter_percentage: 0.0, // No jitter for predictable test
        };

        // Attempt 0: 1000ms
        assert_eq!(
            config.delay_for_attempt(0),
            Some(Duration::from_millis(1000))
        );
        // Attempt 1: 2000ms
        assert_eq!(
            config.delay_for_attempt(1),
            Some(Duration::from_millis(2000))
        );
        // Attempt 2: 4000ms
        assert_eq!(
            config.delay_for_attempt(2),
            Some(Duration::from_millis(4000))
        );
        // Attempt 5: None (max attempts reached)
        assert_eq!(config.delay_for_attempt(5), None);
    }
}
