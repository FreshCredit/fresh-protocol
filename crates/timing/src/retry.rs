// TAG: surface=api owner=platform-team rule=API-001
//! Retry strategies with exponential backoff
//!
//! This module provides retry logic with exponential backoff and jitter
//! to prevent thundering herd problems.

use async_trait::async_trait;
use rand::Rng;
use std::time::Duration;

/// Retry strategy trait
#[async_trait]
pub trait RetryStrategy: Send + Sync {
    /// Get the delay before the next retry attempt
    /// Returns None if max attempts reached
    fn next_delay(&self, attempt: u32) -> Option<Duration>;

    /// Get the maximum number of retry attempts
    fn max_attempts(&self) -> u32;
}

/// Exponential backoff retry strategy
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    /// Initial Delay
    pub initial_delay: Duration,
    /// Max Delay
    // TAG: surface=api owner=platform-team rule=API-001
    pub max_delay: Duration,
    /// Max Attempts
    pub max_attempts: u32,
    /// Jitter Percentage
    pub jitter_percentage: f64,
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(60),
            max_attempts: 3,
            jitter_percentage: 0.2, // ±20%
        }
    }
}

impl ExponentialBackoff {
    /// Create a new exponential backoff strategy
    #[must_use]
    pub const fn new(
        initial_delay: Duration,
        max_delay: Duration,
        max_attempts: u32,
        jitter_percentage: f64,
    ) -> Self {
        // TAG: surface=api owner=platform-team rule=API-001
        Self {
            initial_delay,
            max_delay,
            max_attempts,
            jitter_percentage,
        }
    }

    /// Create from environment variables
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            initial_delay: Duration::from_millis(
                std::env::var("WORKFLOW_RETRY_INITIAL_DELAY_MS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1000),
            ),
            max_delay: Duration::from_millis(
                std::env::var("WORKFLOW_RETRY_MAX_DELAY_MS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(60000),
            ),
            max_attempts: std::env::var("WORKFLOW_RETRY_MAX_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                // TAG: surface=api owner=platform-team rule=API-001
                .unwrap_or(3),
            jitter_percentage: 0.2,
        }
    }
}

impl RetryStrategy for ExponentialBackoff {
    fn next_delay(&self, attempt: u32) -> Option<Duration> {
        if attempt >= self.max_attempts {
            return None;
        }

        // Calculate exponential delay: initial_delay * 2^attempt
        let base_delay = u64::try_from(self.initial_delay.as_millis())
            .expect("initial delay fits in u64")
            .saturating_mul(2u64.pow(attempt));
        let capped_delay = base_delay
            .min(u64::try_from(self.max_delay.as_millis()).expect("max delay fits in u64"));

        // Add jitter to prevent thundering herd
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
        // Safe for bounded jitter calculation
        let jitter_range = (capped_delay as f64 * self.jitter_percentage) as i64;
        let mut rng = rand::thread_rng();
        let jitter = rng.gen_range(-jitter_range..=jitter_range);
        let final_delay = u64::try_from(
            i64::try_from(capped_delay)
                // TAG: surface=api owner=platform-team rule=API-001
                .expect("capped delay fits in i64")
                .saturating_add(jitter)
                .max(0),
        )
        .expect("jittered delay is non-negative");

        Some(Duration::from_millis(final_delay))
    }

    fn max_attempts(&self) -> u32 {
        self.max_attempts
    }
}

/// Retry executor
#[derive(Debug)]
pub struct RetryExecutor<S: RetryStrategy> {
    strategy: S,
}

impl<S: RetryStrategy> RetryExecutor<S> {
    /// Create a new retry executor with the given strategy
    pub const fn new(strategy: S) -> Self {
        Self { strategy }
    }

    // TAG: surface=api owner=platform-team rule=API-001
    /// Execute an operation with retry logic
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn execute<F, Fut, T, E>(&self, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        let mut attempt = 0;

        // SIM-LOOP-001: Bounded retry loop - exits on success or max attempts
        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    if let Some(delay) = self.strategy.next_delay(attempt) {
                        tracing::warn!("Attempt {} failed, retrying in {:?}", attempt + 1, delay);
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                    } else {
                        tracing::error!("All retry attempts exhausted");
                        return Err(err);
                    }
                }
            }
        }
        // TAG: surface=api owner=platform-team rule=API-001
    }
}

/// Convenience wrapper for database operations with default retry logic
///
/// # Example
/// ```ignore
/// let result = with_retry(|| async {
///     db.query("SELECT * FROM users WHERE id = ?", params![user_id]).await
/// }).await?;
/// ```
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn with_retry<F, Fut, T, E>(operation: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let executor = RetryExecutor::new(ExponentialBackoff::default());
    executor.execute(operation).await
}

/// Convenience wrapper with custom max attempts
///
/// # Example
/// ```ignore
// TAG: surface=api owner=platform-team rule=API-001
/// let result = with_retry_attempts(5, || async {
///     external_api.call().await
/// }).await?;
/// ```
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn with_retry_attempts<F, Fut, T, E>(max_attempts: u32, operation: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let strategy = ExponentialBackoff {
        max_attempts,
        ..Default::default()
    };
    let executor = RetryExecutor::new(strategy);
    executor.execute(operation).await
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)]
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());
    // TAG: surface=api owner=platform-team rule=API-001

    #[test]
    fn test_exponential_backoff() {
        let strategy = ExponentialBackoff::default();

        // First attempt: ~1000ms
        let delay1 = strategy.next_delay(0).unwrap();
        assert!(delay1.as_millis() >= 800 && delay1.as_millis() <= 1200);

        // Second attempt: ~2000ms
        let delay2 = strategy.next_delay(1).unwrap();
        assert!(delay2.as_millis() >= 1600 && delay2.as_millis() <= 2400);

        // Third attempt: ~4000ms
        let delay3 = strategy.next_delay(2).unwrap();
        assert!(delay3.as_millis() >= 3200 && delay3.as_millis() <= 4800);

        // Fourth attempt: None (max attempts reached)
        assert!(strategy.next_delay(3).is_none());
    }

    #[test]
    fn test_exponential_backoff_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: Test-only env manipulation. Guarded by ENV_LOCK mutex.
        unsafe {
            // TAG: surface=api owner=platform-team rule=API-001
            std::env::set_var("WORKFLOW_RETRY_INITIAL_DELAY_MS", "500");
            std::env::set_var("WORKFLOW_RETRY_MAX_DELAY_MS", "30000");
            std::env::set_var("WORKFLOW_RETRY_MAX_ATTEMPTS", "5");
        }

        let strategy = ExponentialBackoff::from_env();
        assert_eq!(
            strategy.initial_delay,
            std::time::Duration::from_millis(500)
        );
        assert_eq!(strategy.max_delay, std::time::Duration::from_millis(30000));
        assert_eq!(strategy.max_attempts, 5);

        // SAFETY: Test-only env cleanup. Removes vars set above in same test.
        unsafe {
            std::env::remove_var("WORKFLOW_RETRY_INITIAL_DELAY_MS");
            std::env::remove_var("WORKFLOW_RETRY_MAX_DELAY_MS");
            std::env::remove_var("WORKFLOW_RETRY_MAX_ATTEMPTS");
        }
    }

    #[tokio::test]
    async fn test_retry_executor_success() {
        let strategy = ExponentialBackoff::new(
            std::time::Duration::from_millis(10),
            std::time::Duration::from_millis(100),
            3,
            // TAG: surface=api owner=platform-team rule=API-001
            0.0,
        );
        let executor = RetryExecutor::new(strategy);

        let result = executor.execute(|| async { Ok::<_, String>(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_executor_eventual_success() {
        let strategy = ExponentialBackoff::new(
            std::time::Duration::from_millis(10),
            std::time::Duration::from_millis(100),
            3,
            0.0,
        );
        let executor = RetryExecutor::new(strategy);

        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let result = executor
            .execute(|| {
                let attempts = attempts.clone();
                async move {
                    let count = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    if count < 3 {
                        Err("not yet")
                    } else {
                        // TAG: surface=api owner=platform-team rule=API-001
                        Ok(42)
                    }
                }
            })
            .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_executor_exhausted() {
        let strategy = ExponentialBackoff::new(
            std::time::Duration::from_millis(10),
            std::time::Duration::from_millis(100),
            2,
            0.0,
        );
        let executor = RetryExecutor::new(strategy);

        let result = executor
            .execute(|| async { Err::<i32, _>("always fails") })
            .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "always fails");
    }
    // TAG: surface=api owner=platform-team rule=API-001

    #[tokio::test]
    async fn test_with_retry() {
        let result = with_retry(|| async { Ok::<_, String>(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_retry_attempts() {
        let result = with_retry_attempts(2, || async { Ok::<_, String>(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_retry_attempts_failure() {
        let result = with_retry_attempts(1, || async { Err::<i32, _>("fails") }).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_exponential_backoff_from_env_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        // Ensure env vars are not set so defaults are used
        std::env::remove_var("WORKFLOW_RETRY_INITIAL_DELAY_MS");
        std::env::remove_var("WORKFLOW_RETRY_MAX_DELAY_MS");
        std::env::remove_var("WORKFLOW_RETRY_MAX_ATTEMPTS");
        // TAG: surface=api owner=platform-team rule=API-001

        let strategy = ExponentialBackoff::from_env();
        assert_eq!(
            strategy.initial_delay,
            std::time::Duration::from_millis(1000)
        );
        assert_eq!(strategy.max_delay, std::time::Duration::from_millis(60000));
        assert_eq!(strategy.max_attempts, 3);
        assert_eq!(strategy.jitter_percentage, 0.2);
    }

    #[test]
    fn test_exponential_backoff_new() {
        let strategy = ExponentialBackoff::new(
            std::time::Duration::from_millis(500),
            std::time::Duration::from_secs(30),
            5,
            0.1,
        );
        assert_eq!(
            strategy.initial_delay,
            std::time::Duration::from_millis(500)
        );
        assert_eq!(strategy.max_delay, std::time::Duration::from_secs(30));
        assert_eq!(strategy.max_attempts, 5);
        assert_eq!(strategy.jitter_percentage, 0.1);
    }
    // TAG: surface=api owner=platform-team rule=API-001
}
