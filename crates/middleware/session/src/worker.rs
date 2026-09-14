// TAG: surface=security owner=security-team rule=SEC-001
//! Session cleanup worker
//!
//! This module provides a background worker that periodically cleans up expired sessions.

use crate::config::SessionConfig;
use crate::storage::{SessionError, SessionStorage};

#[cfg(test)]
use crate::storage::SessionArtifact;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info};

/// Shutdown signal type for graceful worker termination
type ShutdownRx = tokio::sync::broadcast::Receiver<()>;

/// Session cleanup worker
#[derive(Debug)]
pub struct SessionCleanupWorker<S: SessionStorage> {
    storage: Arc<S>,
    config: SessionConfig,
}

impl<S: SessionStorage> SessionCleanupWorker<S> {
    /// Create a new session cleanup worker
    pub const fn new(storage: Arc<S>, config: SessionConfig) -> Self {
        Self { storage, config }
    }

    /// Start the cleanup worker
    ///
    /// This runs indefinitely, cleaning up expired sessions at the configured interval.
    /// It should be spawned as a background task.
    // TAG: surface=security owner=platform-team rule=MID-001
    ///
    /// # Arguments
    /// * `shutdown_rx` - Broadcast receiver for graceful shutdown signal
    pub async fn run(self, mut shutdown_rx: ShutdownRx) {
        info!(
            "Starting session cleanup worker (interval: {:?})",
            self.config.cleanup_interval
        );

        let interval_duration = Duration::from_secs(
            self.config
                .cleanup_interval
                .num_seconds()
                .max(0)
                .try_into()
                .unwrap_or(0),
        );

        let mut ticker = interval(interval_duration);

        // P10-R4: intentionally unbounded worker loop — select! terminates on shutdown_rx; each arm is a bounded cleanup call.
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    match self.cleanup().await {
                        Ok(deleted) => {
                            if deleted > 0 {
                                info!("Cleaned up {} expired sessions", deleted);
                            }
                        }
                        Err(err) => {
                            error!("Session cleanup failed: {}", err);
                        }
                    }
                }
                    // TAG: surface=security owner=platform-team rule=GENERAL-001
                _ = shutdown_rx.recv() => {
                    info!("Session cleanup worker received shutdown signal, stopping...");
                    break;
                }
            }
        }

        info!("Session cleanup worker stopped");
    }

    /// Perform a single cleanup operation
    async fn cleanup(&self) -> Result<u64, SessionError> {
        self.storage.cleanup_expired().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Session;
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use std::sync::Mutex;

    struct MockStorage {
        cleanup_count: Arc<Mutex<u64>>,
    }

    #[async_trait]
    impl SessionStorage for MockStorage {
        async fn create(&self, _session: Session) -> Result<(), SessionError> {
            Ok(())
        }
        // TAG: surface=security owner=platform-team rule=MID-001

        async fn get(&self, _session_id: &str) -> Result<Option<Session>, SessionError> {
            Ok(None)
        }

        async fn get_and_refresh(
            &self,
            _session_id: &str,
            _timeout: chrono::Duration,
        ) -> Result<Option<Session>, SessionError> {
            Ok(None)
        }

        async fn update_activity(
            &self,
            _session_id: &str,
            _timeout: chrono::Duration,
        ) -> Result<(), SessionError> {
            Ok(())
        }

        async fn delete(&self, _session_id: &str) -> Result<(), SessionError> {
            Ok(())
        }

        async fn cleanup_expired(&self) -> Result<u64, SessionError> {
            {
                let mut count = self.cleanup_count.lock().unwrap();
                *count += 1;
            }
            Ok(5) // Simulate cleaning up 5 sessions
        }

        async fn get_user_sessions(&self, _user_id: &str) -> Result<Vec<Session>, SessionError> {
            Ok(vec![])
        }
        // TAG: surface=security owner=security-team rule=SEC-001

        async fn record_artifact(&self, _artifact: SessionArtifact) -> Result<(), SessionError> {
            Ok(())
        }

        async fn get_session_artifacts(
            &self,
            _session_id: &str,
        ) -> Result<Vec<SessionArtifact>, SessionError> {
            Ok(vec![])
        }

        async fn get_user_artifacts(
            &self,
            _user_id: &str,
            _limit: i64,
        ) -> Result<Vec<SessionArtifact>, SessionError> {
            Ok(vec![])
        }

        async fn store_refresh_token(
            &self,
            _session_id: &str,
            _refresh_token: &str,
            _expires_at: DateTime<Utc>,
        ) -> Result<(), SessionError> {
            Ok(())
        }

        async fn get_refresh_token(
            &self,
            _session_id: &str,
        ) -> Result<Option<String>, SessionError> {
            // TAG: surface=security owner=platform-team rule=MID-001
            Ok(None)
        }

        async fn update_tokens(
            &self,
            _session_id: &str,
            _access_token: &str,
            _refresh_token: Option<&str>,
            _expires_in: i64,
        ) -> Result<(), SessionError> {
            Ok(())
        }

        async fn get_by_access_token(
            &self,
            _access_token: &str,
        ) -> Result<Option<Session>, SessionError> {
            Ok(None)
        }

        #[allow(clippy::too_many_arguments)]
        async fn record_refresh_event(
            &self,
            _session_id: &str,
            _user_id: &str,
            _event_type: &str,
            _ip_address: Option<&str>,
            _user_agent: Option<&str>,
            _success: bool,
            _error_message: Option<&str>,
        ) -> Result<(), SessionError> {
            Ok(())
        }

        // TAG: surface=security owner=platform-team rule=GENERAL-001
        async fn check_refresh_rate_limit(
            &self,
            _session_id: &str,
            _ip_address: &str,
        ) -> Result<bool, SessionError> {
            Ok(true)
        }

        async fn increment_refresh_rate_limit(
            &self,
            _session_id: &str,
            _ip_address: &str,
        ) -> Result<(), SessionError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_cleanup_worker() {
        let cleanup_count = Arc::new(Mutex::new(0u64));
        let storage = Arc::new(MockStorage {
            cleanup_count: cleanup_count.clone(),
        });

        let config = SessionConfig {
            cleanup_interval: chrono::Duration::milliseconds(100),
            ..Default::default()
        };

        let worker = SessionCleanupWorker::new(storage, config);

        // Run cleanup once
        let deleted = worker.cleanup().await.unwrap();
        // TAG: surface=security owner=platform-team rule=MID-001
        assert_eq!(deleted, 5);

        assert_eq!(*cleanup_count.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_run_loop_with_shutdown() {
        let cleanup_count = Arc::new(Mutex::new(0u64));
        let storage = Arc::new(MockStorage {
            cleanup_count: cleanup_count.clone(),
        });

        let config = SessionConfig {
            cleanup_interval: chrono::Duration::seconds(1),
            ..Default::default()
        };

        let worker = SessionCleanupWorker::new(storage, config);

        let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

        let handle = tokio::spawn(async move {
            worker.run(shutdown_rx).await;
        });

        // Send shutdown immediately - worker should exit before first tick
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let _ = shutdown_tx.send(());

        // Wait for worker to finish with timeout
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(2), handle).await;
        assert!(result.is_ok(), "Worker did not shut down in time");
    }
    // TAG: surface=security owner=security-team rule=SEC-001
}
