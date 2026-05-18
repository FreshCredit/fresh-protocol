//! Timeout enforcement for async operations
//!
//! This module provides utilities for enforcing timeouts on async operations.

use std::time::Duration;
use tokio::time::timeout;

/// Timeout-related errors
#[derive(Debug, thiserror::Error)]
pub enum TimeoutError<E> {
    #[error("Operation timed out after {0:?}")]
    /// Timeout
    Timeout(Duration),

    #[error("Operation failed: {0}")]
    /// Operationerror
    OperationError(E),
}

/// Timeout enforcer
pub struct TimeoutEnforcer {
    default_timeout: Duration,
    max_timeout: Duration,
}

impl Default for TimeoutEnforcer {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            max_timeout: Duration::from_secs(300),
        }
    }
}

impl TimeoutEnforcer {
    /// Create a new timeout enforcer
    #[must_use]
    pub const fn new(default_timeout: Duration, max_timeout: Duration) -> Self {
        Self {
            default_timeout,
            max_timeout,
        }
    }

    /// Create from environment variables
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            default_timeout: Duration::from_millis(
                std::env::var("WORKFLOW_DEFAULT_TIMEOUT_MS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(30000),
            ),
            max_timeout: Duration::from_millis(
                std::env::var("WORKFLOW_MAX_TIMEOUT_MS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(300_000),
            ),
        }
    }

    /// Execute an operation with timeout enforcement
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn execute<F, Fut, T, E>(
        &self,
        operation: F,
        timeout_duration: Option<Duration>,
    ) -> Result<T, TimeoutError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        let timeout_duration = timeout_duration
            .unwrap_or(self.default_timeout)
            .min(self.max_timeout);

        match timeout(timeout_duration, operation()).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(err)) => Err(TimeoutError::OperationError(err)),
            Err(_) => Err(TimeoutError::Timeout(timeout_duration)),
        }
    }

    /// Execute an operation with the default timeout
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn execute_default<F, Fut, T, E>(&self, operation: F) -> Result<T, TimeoutError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        self.execute(operation, None).await
    }
}

#[cfg(test)]
mod tests {
    #![allow(unsafe_code)]
    use super::*;
    use std::sync::Mutex;
    use tokio::time::sleep;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[tokio::test]
    async fn test_timeout_success() {
        let enforcer = TimeoutEnforcer::default();

        let result = enforcer
            .execute(
                || async { Ok::<_, String>(42) },
                Some(Duration::from_secs(1)),
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_timeout_exceeded() {
        let enforcer = TimeoutEnforcer::default();

        let result = enforcer
            .execute(
                || async {
                    sleep(Duration::from_secs(2)).await;
                    Ok::<_, String>(42)
                },
                Some(Duration::from_millis(100)),
            )
            .await;

        assert!(result.is_err());
        match result {
            Err(TimeoutError::Timeout(d)) => assert_eq!(d, Duration::from_millis(100)),
            _ => panic!("Expected timeout error"),
        }
    }

    #[tokio::test]
    async fn test_operation_error() {
        let enforcer = TimeoutEnforcer::default();

        let result = enforcer
            .execute(
                || async { Err::<i32, _>("operation failed") },
                Some(Duration::from_secs(1)),
            )
            .await;

        assert!(result.is_err());
        match result {
            Err(TimeoutError::OperationError(e)) => assert_eq!(e, "operation failed"),
            _ => panic!("Expected operation error"),
        }
    }

    #[test]
    fn test_timeout_enforcer_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("WORKFLOW_DEFAULT_TIMEOUT_MS", "15000");
            std::env::set_var("WORKFLOW_MAX_TIMEOUT_MS", "600000");
        }

        let enforcer = TimeoutEnforcer::from_env();
        assert_eq!(enforcer.default_timeout, Duration::from_millis(15000));
        assert_eq!(enforcer.max_timeout, Duration::from_millis(600000));

        unsafe {
            std::env::remove_var("WORKFLOW_DEFAULT_TIMEOUT_MS");
            std::env::remove_var("WORKFLOW_MAX_TIMEOUT_MS");
        }
    }

    #[tokio::test]
    async fn test_execute_default_timeout() {
        let enforcer = TimeoutEnforcer::new(Duration::from_millis(500), Duration::from_secs(10));

        let result = enforcer
            .execute_default(|| async {
                sleep(Duration::from_millis(50)).await;
                Ok::<_, String>(42)
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_timeout_with_max_cap() {
        let enforcer = TimeoutEnforcer::new(Duration::from_secs(1), Duration::from_millis(100));

        // Requested timeout (1s) exceeds max (100ms), should be capped
        let result = enforcer
            .execute(
                || async {
                    sleep(Duration::from_millis(200)).await;
                    Ok::<_, String>(42)
                },
                Some(Duration::from_secs(1)),
            )
            .await;

        assert!(result.is_err());
        match result {
            Err(TimeoutError::Timeout(d)) => assert_eq!(d, Duration::from_millis(100)),
            _ => panic!("Expected timeout capped at max"),
        }
    }

    #[test]
    fn test_timeout_enforcer_from_env_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("WORKFLOW_DEFAULT_TIMEOUT_MS");
        std::env::remove_var("WORKFLOW_MAX_TIMEOUT_MS");

        let enforcer = TimeoutEnforcer::from_env();
        assert_eq!(enforcer.default_timeout, Duration::from_millis(30000));
        assert_eq!(enforcer.max_timeout, Duration::from_millis(300_000));
    }

    #[test]
    fn test_timeout_enforcer_new() {
        let enforcer = TimeoutEnforcer::new(Duration::from_secs(10), Duration::from_secs(60));
        assert_eq!(enforcer.default_timeout, Duration::from_secs(10));
        assert_eq!(enforcer.max_timeout, Duration::from_secs(60));
    }
}
