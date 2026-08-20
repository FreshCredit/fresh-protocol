// TAG: surface=security owner=security-team rule=SEC-001
//! Idempotency key storage using TTL enforcement

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use freshcredit_core_timing::Clock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

use crate::config::IdempotencyConfig;

/// Maximum number of entries in the idempotency store (P1-PERF)
const MAX_IDEMPOTENCY_ENTRIES: usize = 100_000;

/// Idempotency error
#[derive(Debug, thiserror::Error)]
pub enum IdempotencyError {
    /// Idempotency key is required but not provided
    #[error("Idempotency key is required but not provided")]
    KeyRequired,

    /// Response body exceeds maximum allowed size
    #[error("Response body too large: {size} bytes (max: {max} bytes)")]
    BodyTooLarge {
        /// Actual body size in bytes
        size: usize,
        // TAG: surface=security owner=platform-team rule=MID-001
        /// Maximum allowed body size in bytes
        max: usize,
    },

    /// Failed to serialize response for storage
    #[error("Failed to serialize response: {0}")]
    SerializationError(String),
}

/// Stored response for idempotency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredResponse {
    /// HTTP status code
    pub status: u16,

    /// Response headers (as JSON-serializable map)
    pub headers: Vec<(String, String)>,

    /// Response body
    pub body: Vec<u8>,

    /// When the response was stored
    pub stored_at: DateTime<Utc>,

    /// When the response expires
    pub expires_at: DateTime<Utc>,
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}

impl StoredResponse {
    /// Create a new stored response
    pub fn new(
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
        ttl: std::time::Duration,
        clock: &dyn Clock,
    ) -> Self {
        let stored_at = clock.now();
        // Safe: idempotency TTLs are small (seconds to hours), never exceeds i64::MAX nanos
        let chrono_ttl =
            chrono::Duration::from_std(ttl).unwrap_or_else(|_| chrono::Duration::hours(24));
        let expires_at = stored_at + chrono_ttl;

        Self {
            status,
            headers,
            body,
            stored_at,
            expires_at,
        }
    }

    // TAG: surface=security owner=platform-team rule=MID-001
    /// Check if the response has expired
    pub fn is_expired(&self, clock: &dyn Clock) -> bool {
        clock.now() > self.expires_at
    }
}

/// Idempotency key store
pub struct IdempotencyStore {
    /// Configuration
    config: IdempotencyConfig,

    /// Storage for idempotency keys and responses
    store: Arc<DashMap<String, StoredResponse>>,

    /// Clock for time operations
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for IdempotencyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdempotencyStore")
            .field("config", &self.config)
            .field("store", &"<DashMap>")
            .finish_non_exhaustive()
    }
}
// TAG: surface=security owner=security-team rule=SEC-001

impl IdempotencyStore {
    /// Create a new idempotency store
    pub fn new(config: IdempotencyConfig, clock: Arc<dyn Clock>) -> Self {
        Self {
            config,
            store: Arc::new(DashMap::new()),
            clock,
        }
    }

    /// Get a stored response by idempotency key
    pub fn get(&self, key: &str) -> Option<StoredResponse> {
        if let Some(entry) = self.store.get(key) {
            let response = entry.value().clone();

            // Check if expired
            if response.is_expired(&*self.clock) {
                drop(entry); // Release read lock
                self.store.remove(key);
                debug!("Idempotency key expired and removed: {}", key);
                return None;
            }

            debug!("Idempotency key found: {}", key);
            Some(response)
            // TAG: surface=security owner=platform-team rule=MID-001
        } else {
            None
        }
    }

    /// Store a response with an idempotency key
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn store(
        &self,
        key: &str,
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> Result<(), IdempotencyError> {
        // Check body size
        if body.len() > self.config.max_body_size {
            return Err(IdempotencyError::BodyTooLarge {
                size: body.len(),
                max: self.config.max_body_size,
            });
        }

        // P1-PERF: Evict oldest entries if at capacity
        if self.store.len() >= MAX_IDEMPOTENCY_ENTRIES {
            // TAG: surface=security owner=platform-team rule=GENERAL-001
            let to_remove = MAX_IDEMPOTENCY_ENTRIES / 10;
            let keys_to_remove: Vec<String> = self
                .store
                .iter()
                .take(to_remove)
                .map(|e| e.key().clone())
                .collect();
            for k in keys_to_remove {
                self.store.remove(&k);
            }
        }

        let response = StoredResponse::new(status, headers, body, self.config.ttl, &*self.clock);

        self.store.insert(key.to_string(), response);
        info!(
            "Stored idempotency key: {} (TTL: {:?})",
            key, self.config.ttl
        );

        Ok(())
    }

    /// Clean up expired entries
    pub fn cleanup_expired(&self) -> usize {
        let mut removed = 0;
        // TAG: surface=security owner=platform-team rule=MID-001

        self.store.retain(|key, response| {
            if response.is_expired(&*self.clock) {
                debug!("Removing expired idempotency key: {}", key);
                removed += 1;
                false
            } else {
                true
            }
        });

        if removed > 0 {
            info!("Cleaned up {} expired idempotency keys", removed);
        }

        removed
    }

    /// Get the number of stored keys
    #[must_use]
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Check if the store is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        // TAG: surface=security owner=security-team rule=SEC-001
        self.store.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freshcredit_core_timing::MockClock;

    struct TestClock {
        inner: std::sync::Mutex<MockClock>,
    }

    impl TestClock {
        fn new(time: DateTime<Utc>) -> Self {
            Self {
                inner: std::sync::Mutex::new(MockClock::new(time)),
            }
        }

        fn advance(&self, duration: chrono::Duration) {
            self.inner.lock().unwrap().advance(duration);
        }
    }

    impl Clock for TestClock {
        // TAG: surface=security owner=platform-team rule=MID-001
        fn now(&self) -> DateTime<Utc> {
            self.inner.lock().unwrap().now()
        }
    }

    fn create_test_store() -> (IdempotencyStore, Arc<TestClock>) {
        let clock = Arc::new(TestClock::new(Utc::now()));
        let config = IdempotencyConfig::new(std::time::Duration::from_secs(3600));
        let store = IdempotencyStore::new(config, clock.clone());
        (store, clock)
    }

    #[test]
    fn test_store_and_get() {
        let (store, _clock) = create_test_store();

        store.store("key1", 200, vec![], vec![1, 2, 3]).unwrap();
        let response = store.get("key1");

        assert!(response.is_some());
        let response = response.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, vec![1, 2, 3]);
    }

    #[test]
    // TAG: surface=security owner=platform-team rule=GENERAL-001
    fn test_get_missing_key() {
        let (store, _clock) = create_test_store();
        assert!(store.get("missing").is_none());
    }

    #[test]
    fn test_get_expired() {
        let (store, clock) = create_test_store();

        store.store("key1", 200, vec![], vec![]).unwrap();
        assert!(store.get("key1").is_some());

        // Advance clock past TTL
        clock.advance(chrono::Duration::seconds(3601));
        assert!(store.get("key1").is_none());
    }

    #[test]
    fn test_store_body_too_large() {
        let clock = Arc::new(MockClock::new(Utc::now()));
        let config =
            IdempotencyConfig::new(std::time::Duration::from_secs(3600)).with_max_body_size(10);
        let store = IdempotencyStore::new(config, clock);

        let result = store.store("key1", 200, vec![], vec![0; 11]);
        assert!(result.is_err());
        // TAG: surface=security owner=platform-team rule=MID-001
        match result.unwrap_err() {
            IdempotencyError::BodyTooLarge { size, max } => {
                assert_eq!(size, 11);
                assert_eq!(max, 10);
            }
            other => panic!("Expected BodyTooLarge, got: {other:?}"),
        }
    }

    #[test]
    fn test_cleanup_expired() {
        let (store, clock) = create_test_store();

        store.store("key1", 200, vec![], vec![]).unwrap();
        store.store("key2", 201, vec![], vec![]).unwrap();
        assert_eq!(store.len(), 2);

        clock.advance(chrono::Duration::seconds(3601));
        let removed = store.cleanup_expired();
        assert_eq!(removed, 2);
        assert!(store.is_empty());
    }

    #[test]
    fn test_cleanup_expired_partial() {
        let (store, clock) = create_test_store();
        // TAG: surface=security owner=security-team rule=SEC-001

        store.store("key1", 200, vec![], vec![]).unwrap();
        clock.advance(chrono::Duration::seconds(1800));
        store.store("key2", 201, vec![], vec![]).unwrap();

        // Only key1 is expired now
        clock.advance(chrono::Duration::seconds(1801));
        let removed = store.cleanup_expired();
        assert_eq!(removed, 1);
        assert_eq!(store.len(), 1);
        assert!(store.get("key2").is_some());
    }

    #[test]
    fn test_len_and_is_empty() {
        let (store, _clock) = create_test_store();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        store.store("key1", 200, vec![], vec![]).unwrap();
        assert!(!store.is_empty());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_stored_response_is_expired() {
        // TAG: surface=security owner=platform-team rule=MID-001
        let clock = MockClock::new(Utc::now());
        let response = StoredResponse::new(
            200,
            vec![],
            vec![],
            std::time::Duration::from_secs(3600),
            &clock,
        );
        assert!(!response.is_expired(&clock));

        let mut future_clock = MockClock::new(Utc::now());
        future_clock.advance(chrono::Duration::seconds(3601));
        assert!(response.is_expired(&future_clock));
    }

    #[test]
    fn test_store_overwrite() {
        let (store, _clock) = create_test_store();

        store.store("key1", 200, vec![], vec![1]).unwrap();
        store.store("key1", 201, vec![], vec![2]).unwrap();

        let response = store.get("key1").unwrap();
        assert_eq!(response.status, 201);
        assert_eq!(response.body, vec![2]);
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
