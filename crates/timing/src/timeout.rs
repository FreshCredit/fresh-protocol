//! Timeout enforcement for async operations
//!
//! This module provides utilities for enforcing timeouts on async operations.

use std::time::Duration;
use tokio::time::timeout;

/// Timeout-related errors
#[derive(Debug, thiserror::Error)]
pub enum TimeoutError<E> {
    #[error("Operation timed out after {0:?}")]
    Timeout(Duration),

    #[error("Operation failed: {0}")]
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
    pub fn new(default_timeout: Duration, max_timeout: Duration) -> Self {
        Self {
            default_timeout,
            max_timeout,
        }
    }

    /// Create from environment variables
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
                    .unwrap_or(300000),
            ),
        }
    }

    /// Execute an operation with timeout enforcement
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
    use super::*;
    use tokio::time::sleep;

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
}
