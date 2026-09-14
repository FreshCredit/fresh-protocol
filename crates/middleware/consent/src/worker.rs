// TAG: surface=security owner=security-team rule=SEC-001
//! Consent cleanup worker
//!
//! This module provides a background worker that periodically marks expired consents as revoked.

use crate::storage::ConsentStorage;
use crate::types::ConsentError;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info};

/// Shutdown signal type for graceful worker termination
type ShutdownRx = tokio::sync::broadcast::Receiver<()>;

/// Consent cleanup worker
#[derive(Debug)]
pub struct ConsentCleanupWorker<S: ConsentStorage> {
    storage: Arc<S>,
    cleanup_interval: Duration,
}

impl<S: ConsentStorage> ConsentCleanupWorker<S> {
    /// Create a new consent cleanup worker
    pub const fn new(storage: Arc<S>, cleanup_interval: Duration) -> Self {
        Self {
            storage,
            cleanup_interval,
        }
    }

    /// Create with default interval (1 hour)
    // TAG: surface=security owner=platform-team rule=MID-001
    pub const fn with_default_interval(storage: Arc<S>) -> Self {
        Self::new(storage, Duration::from_secs(3600))
    }

    /// Start the cleanup worker
    ///
    /// This runs indefinitely, cleaning up expired consents at the configured interval.
    /// It should be spawned as a background task.
    ///
    /// # Arguments
    /// * `shutdown_rx` - Broadcast receiver for graceful shutdown signal
    pub async fn run(self, mut shutdown_rx: ShutdownRx) {
        info!(
            "Starting consent cleanup worker (interval: {:?})",
            self.cleanup_interval
        );

        let mut ticker = interval(self.cleanup_interval);

        // P10-R4: intentionally unbounded worker loop — select! terminates on shutdown_rx; each arm is a bounded cleanup call.
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    match self.cleanup().await {
                        Ok(revoked) => {
                            if revoked > 0 {
                                info!("Marked {} expired consents as revoked", revoked);
                            }
                        }
                        Err(err) => {
                            error!("Consent cleanup failed: {}", err);
                        // TAG: surface=security owner=platform-team rule=GENERAL-001
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Consent cleanup worker received shutdown signal, stopping...");
                    break;
                }
            }
        }

        info!("Consent cleanup worker stopped");
    }

    /// Perform a single cleanup operation
    async fn cleanup(&self) -> Result<u64, ConsentError> {
        self.storage.cleanup_expired().await
    }
}

#[cfg(test)]
#[allow(unused_variables, unused_imports)]
mod tests {
    use super::*;
    use crate::types::{Consent, ConsentType, Jurisdiction};
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct MockStorage {
        cleanup_count: Arc<Mutex<u64>>,
    }
    // TAG: surface=security owner=platform-team rule=MID-001

    #[async_trait]
    impl ConsentStorage for MockStorage {
        async fn get_consent(
            &self,
            _user_id: i64,
            _consent_type: &ConsentType,
        ) -> Result<Option<Consent>, ConsentError> {
            Ok(None)
        }

        async fn create_consent(
            &self,
            user_id: i64,
            consent_type: ConsentType,
            consent_purpose: String,
            consent_method: String,
            jurisdiction: Jurisdiction,
            expiry_timestamp: Option<chrono::DateTime<chrono::Utc>>,
        ) -> Result<Consent, ConsentError> {
            // Mock implementation - returns a consent record
            Ok(Consent {
                id: 1,
                user_id,
                consent_type,
                consent_purpose,
                consent_granted: true,
                consent_method,
                consent_timestamp: chrono::Utc::now(),
                expiry_timestamp,
                // TAG: surface=security owner=security-team rule=SEC-001
                revoked: false,
                revocation_timestamp: None,
                jurisdiction,
            })
        }

        async fn revoke_consent(
            &self,
            _user_id: i64,
            _consent_type: &ConsentType,
        ) -> Result<(), ConsentError> {
            Ok(())
        }

        async fn get_user_consents(&self, _user_id: i64) -> Result<Vec<Consent>, ConsentError> {
            Ok(vec![])
        }

        async fn cleanup_expired(&self) -> Result<u64, ConsentError> {
            *self.cleanup_count.lock().unwrap() += 1;
            Ok(3) // Simulate cleaning up 3 consents
        }
    }

    #[tokio::test]
    async fn test_cleanup_worker() {
        let cleanup_count = Arc::new(Mutex::new(0u64));
        let storage = Arc::new(MockStorage {
            // TAG: surface=security owner=platform-team rule=MID-001
            cleanup_count: cleanup_count.clone(),
        });

        let worker = ConsentCleanupWorker::new(storage, Duration::from_millis(100));

        // Run cleanup once
        let revoked = worker.cleanup().await.unwrap();
        assert_eq!(revoked, 3);

        assert_eq!(*cleanup_count.lock().unwrap(), 1);
    }

    #[test]
    fn test_worker_new() {
        let storage = Arc::new(MockStorage {
            cleanup_count: Arc::new(Mutex::new(0)),
        });
        let worker = ConsentCleanupWorker::new(storage, Duration::from_secs(60));
        // Just verify it compiles and creates successfully
    }

    #[test]
    fn test_worker_with_default_interval() {
        let storage = Arc::new(MockStorage {
            cleanup_count: Arc::new(Mutex::new(0)),
        });
        let worker = ConsentCleanupWorker::with_default_interval(storage);
        // Just verify it compiles and creates successfully
    }
    // TAG: surface=security owner=platform-team rule=GENERAL-001
}
