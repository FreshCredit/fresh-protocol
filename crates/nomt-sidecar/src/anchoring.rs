// TAG: surface=blockchain owner=blockchain-team rule=BC-001
//! NOMT Root Anchoring to Substrate
//!
//! This module handles periodic anchoring of the NOMT root hash to the Substrate blockchain.
//! The anchoring process:
//! 1. Monitors NOMT storage for new hashes
//! 2. Periodically (every N stores or M minutes) anchors the current root to Substrate
//! 3. Uses the blockchain-client HTTP API for anchoring (not direct RPC)
//!
//! Architecture:
//! - NOMT sidecar generates proofs and stores data locally
//! - Root hash is anchored to Substrate for trustless verification
//! - Browser can verify proofs offline using cached NOMT proofs
//! - Substrate provides the anchor point for root verification

use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Metrics for anchoring operations
#[derive(Default)]
pub struct AnchoringMetrics {
    /// Total anchoring attempts
    pub total_attempts: AtomicU64,
    /// Successful anchoring operations
    pub successful_anchors: AtomicU64,
    /// Failed anchoring operations
    pub failed_anchors: AtomicU64,
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    /// Total latency in milliseconds (for averaging)
    pub total_latency_ms: AtomicU64,
}

/// Configuration for NOMT root anchoring
#[derive(Clone)]
pub struct AnchoringConfig {
    /// Substrate node RPC URL (for direct anchoring) - reserved for future use
    #[allow(dead_code)]
    pub substrate_rpc_url: String,
    /// Blockchain API URL (for HTTP-based anchoring)
    pub blockchain_api_url: String,
    /// Anchor after this many new stores
    pub anchor_interval_stores: u64,
    /// Anchor after this many seconds (even if no new stores)
    pub anchor_interval_seconds: u64,
    /// Enable anchoring (can be disabled for testing)
    pub enabled: bool,
}

impl Default for AnchoringConfig {
    fn default() -> Self {
        Self {
            substrate_rpc_url: "http://localhost:9944".to_string(),
            blockchain_api_url: "http://localhost:8080".to_string(),
            anchor_interval_stores: 10,
            anchor_interval_seconds: 300, // 5 minutes
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            enabled: true,
        }
    }
}

/// NOMT Root Anchoring Service
pub struct AnchoringService {
    config: AnchoringConfig,
    http_client: reqwest::Client,
    stores_since_anchor: AtomicU64,
    last_anchored_root: RwLock<Option<String>>,
    last_anchor_time: RwLock<std::time::Instant>,
    metrics: AnchoringMetrics,
}

impl AnchoringService {
    /// Create a new anchoring service
    pub fn new(config: AnchoringConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
            stores_since_anchor: AtomicU64::new(0),
            last_anchored_root: RwLock::new(None),
            last_anchor_time: RwLock::new(std::time::Instant::now()),
            metrics: AnchoringMetrics::default(),
        }
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001

    /// Notify the service that a new hash was stored
    /// Returns true if anchoring should be triggered
    pub async fn notify_store(&self) -> bool {
        if !self.config.enabled {
            return false;
        }

        let count = self.stores_since_anchor.fetch_add(1, Ordering::Relaxed) + 1;

        // Check if we should anchor based on store count
        if count >= self.config.anchor_interval_stores {
            debug!(count = count, "Anchor threshold reached by store count");
            return true;
        }

        // Check if we should anchor based on time
        let last_time = *self.last_anchor_time.read().await;
        if last_time.elapsed().as_secs() >= self.config.anchor_interval_seconds {
            debug!(
                elapsed_secs = last_time.elapsed().as_secs(),
                "Anchor threshold reached by time"
            );
            return true;
        }

        false
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    }

    /// Anchor the current NOMT root to Substrate via HTTP API
    pub async fn anchor_root(
        &self,
        current_root: &str,
        user_id: &str,
    ) -> anyhow::Result<AnchorResult> {
        if !self.config.enabled {
            return Ok(AnchorResult {
                success: false,
                message: "Anchoring disabled".to_string(),
                block_number: None,
                tx_hash: None,
            });
        }

        // Track metrics
        self.metrics.total_attempts.fetch_add(1, Ordering::Relaxed);
        let start = std::time::Instant::now();

        info!(root = %current_root, "Anchoring NOMT root to Substrate");

        // Use the blockchain API to anchor the hash
        // The blockchain API expects: user_id, report_type, report_data
        // We send the NOMT root as report_data with report_type = "nomt_root"
        let anchor_url = format!("{}/anchor", self.config.blockchain_api_url);
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001

        let response = self
            .http_client
            .post(&anchor_url)
            .json(&serde_json::json!({
                "user_id": user_id,
                "report_type": "nomt_root",
                "report_data": current_root
            }))
            .send()
            .await;

        // Record latency
        let latency_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.metrics
            .total_latency_ms
            .fetch_add(latency_ms, Ordering::Relaxed);

        match response {
            Ok(resp) if resp.status().is_success() => {
                let body: serde_json::Value = resp.json().await.unwrap_or_default();

                // Reset counters
                self.stores_since_anchor.store(0, Ordering::Relaxed);
                *self.last_anchor_time.write().await = std::time::Instant::now();
                *self.last_anchored_root.write().await = Some(current_root.to_string());

                // TAG: surface=blockchain owner=blockchain-team rule=BC-001
                // Track success
                self.metrics
                    .successful_anchors
                    .fetch_add(1, Ordering::Relaxed);

                info!(
                    root = %current_root,
                    block_number = ?body.get("block_number"),
                    latency_ms = latency_ms,
                    "Successfully anchored NOMT root to Substrate"
                );

                Ok(AnchorResult {
                    success: true,
                    message: "Root anchored successfully".to_string(),
                    block_number: body.get("block_number").and_then(serde_json::Value::as_u64),
                    tx_hash: body
                        .get("tx_hash")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                })
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                self.metrics.failed_anchors.fetch_add(1, Ordering::Relaxed);
                warn!(status = %status, body = %body, "Failed to anchor root");
                // TAG: surface=blockchain owner=blockchain-team rule=BC-001
                Ok(AnchorResult {
                    success: false,
                    message: format!("HTTP {status}: {body}"),
                    block_number: None,
                    tx_hash: None,
                })
            }
            Err(e) => {
                self.metrics.failed_anchors.fetch_add(1, Ordering::Relaxed);
                error!(error = %e, "Network error during anchoring");
                Ok(AnchorResult {
                    success: false,
                    message: format!("Network error: {e}"),
                    block_number: None,
                    tx_hash: None,
                })
            }
        }
    }

    /// Get the last anchored root
    pub async fn get_last_anchored_root(&self) -> Option<String> {
        self.last_anchored_root.read().await.clone()
    }

    /// Get anchoring statistics
    pub async fn get_stats(&self) -> serde_json::Value {
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        let last_root = self.last_anchored_root.read().await.clone();
        let last_time = self.last_anchor_time.read().await;

        serde_json::json!({
            "enabled": self.config.enabled,
            "stores_since_anchor": self.stores_since_anchor.load(Ordering::Relaxed),
            "anchor_interval_stores": self.config.anchor_interval_stores,
            "anchor_interval_seconds": self.config.anchor_interval_seconds,
            "last_anchored_root": last_root,
            "seconds_since_anchor": last_time.elapsed().as_secs()
        })
    }

    /// Get Prometheus-formatted metrics for anchoring
    pub fn get_prometheus_metrics(&self) -> String {
        let total_attempts = self.metrics.total_attempts.load(Ordering::Relaxed);
        let successful = self.metrics.successful_anchors.load(Ordering::Relaxed);
        let failed = self.metrics.failed_anchors.load(Ordering::Relaxed);
        let total_latency = self.metrics.total_latency_ms.load(Ordering::Relaxed);
        let avg_latency = if total_attempts > 0 {
            total_latency / total_attempts
        } else {
            0
        };
        let stores_pending = self.stores_since_anchor.load(Ordering::Relaxed);
        let enabled = i32::from(self.config.enabled);

        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        format!(
            "# HELP nomt_anchor_attempts_total Total anchoring attempts\n\
             # TYPE nomt_anchor_attempts_total counter\n\
             nomt_anchor_attempts_total {total_attempts}\n\
             # HELP nomt_anchor_success_total Successful anchoring operations\n\
             # TYPE nomt_anchor_success_total counter\n\
             nomt_anchor_success_total {successful}\n\
             # HELP nomt_anchor_failures_total Failed anchoring operations\n\
             # TYPE nomt_anchor_failures_total counter\n\
             nomt_anchor_failures_total {failed}\n\
             # HELP nomt_anchor_latency_avg_ms Average anchoring latency in milliseconds\n\
             # TYPE nomt_anchor_latency_avg_ms gauge\n\
             nomt_anchor_latency_avg_ms {avg_latency}\n\
             # HELP nomt_anchor_stores_pending Stores since last anchor\n\
             # TYPE nomt_anchor_stores_pending gauge\n\
             nomt_anchor_stores_pending {stores_pending}\n\
             # HELP nomt_anchor_enabled Anchoring enabled status\n\
             # TYPE nomt_anchor_enabled gauge\n\
             nomt_anchor_enabled {enabled}\n"
        )
    }
}

/// Result of an anchoring operation
#[derive(Debug, Clone, serde::Serialize)]
pub struct AnchorResult {
    pub success: bool,
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    pub message: String,
    pub block_number: Option<u64>,
    pub tx_hash: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_notify_store_disabled() {
        let config = AnchoringConfig {
            enabled: false,
            ..Default::default()
        };
        let service = AnchoringService::new(config);
        for _ in 0..3 {
            assert!(!service.notify_store().await);
        }
    }

    #[tokio::test]
    async fn test_notify_store_count_threshold() {
        let config = AnchoringConfig {
            enabled: true,
            anchor_interval_stores: 3,
            anchor_interval_seconds: 3600,
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            ..Default::default()
        };
        let service = AnchoringService::new(config);
        assert!(!service.notify_store().await); // 1
        assert!(!service.notify_store().await); // 2
        assert!(service.notify_store().await); // 3 >= threshold
    }

    #[tokio::test]
    async fn test_notify_store_below_threshold() {
        let config = AnchoringConfig {
            enabled: true,
            anchor_interval_stores: 10,
            anchor_interval_seconds: 3600,
            ..Default::default()
        };
        let service = AnchoringService::new(config);
        for _ in 0..5 {
            assert!(!service.notify_store().await);
        }
    }

    #[tokio::test]
    async fn test_get_stats_reflects_config() {
        let config = AnchoringConfig {
            enabled: true,
            anchor_interval_stores: 42,
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            anchor_interval_seconds: 123,
            ..Default::default()
        };
        let service = AnchoringService::new(config);
        let stats = service.get_stats().await;
        assert_eq!(stats["enabled"], true);
        assert_eq!(stats["anchor_interval_stores"], 42);
        assert_eq!(stats["anchor_interval_seconds"], 123);
    }

    #[tokio::test]
    async fn test_get_stats_stores_since_anchor() {
        let config = AnchoringConfig {
            enabled: true,
            anchor_interval_stores: 100,
            anchor_interval_seconds: 3600,
            ..Default::default()
        };
        let service = AnchoringService::new(config);
        let stats_before = service.get_stats().await;
        assert_eq!(stats_before["stores_since_anchor"], 0);

        service.notify_store().await;
        let stats_after = service.get_stats().await;
        assert_eq!(stats_after["stores_since_anchor"], 1);
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    #[test]
    fn test_get_prometheus_metrics_format() {
        let config = AnchoringConfig::default();
        let service = AnchoringService::new(config);
        let metrics = service.get_prometheus_metrics();
        assert!(metrics.contains("nomt_anchor_attempts_total"));
        assert!(metrics.contains("nomt_anchor_success_total"));
        assert!(metrics.contains("nomt_anchor_failures_total"));
    }

    #[test]
    fn test_get_prometheus_metrics_zero_state() {
        let config = AnchoringConfig::default();
        let service = AnchoringService::new(config);
        let metrics = service.get_prometheus_metrics();
        assert!(metrics.contains("nomt_anchor_attempts_total 0"));
        assert!(metrics.contains("nomt_anchor_success_total 0"));
        assert!(metrics.contains("nomt_anchor_failures_total 0"));
    }

    #[tokio::test]
    async fn test_get_last_anchored_root_initially_none() {
        let config = AnchoringConfig::default();
        let service = AnchoringService::new(config);
        let root = service.get_last_anchored_root().await;
        assert!(root.is_none());
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
