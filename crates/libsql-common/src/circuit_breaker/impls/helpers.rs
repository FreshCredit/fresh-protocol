use super::*;

/// Map a [`CircuitBreakerError`] to an [`anyhow::Error`] with human-friendly
/// messages for the open / half-open limit variants.
pub fn map_cb_result<T>(result: Result<T, CircuitBreakerError>) -> anyhow::Result<T> {
    match result {
        Ok(v) => Ok(v),
        Err(CircuitBreakerError::CircuitOpen) => Err(anyhow::anyhow!(
            "Database circuit breaker is OPEN - service temporarily unavailable"
        )),
        Err(CircuitBreakerError::CircuitHalfOpenLimit) => Err(anyhow::anyhow!(
            "Database circuit breaker is HALF_OPEN - too many concurrent test calls"
        )),
        Err(CircuitBreakerError::DatabaseError(e)) => Err(e),
    }
}
