// TAG: surface=blockchain owner=blockchain-team rule=BC-001
//! Peer Sync Module for NOMT Sidecar
//!
//! Enables synchronization between `StatefulSet` replicas via HTTP.
//! Uses Kubernetes headless service DNS for peer discovery.
//!
//! Architecture:
//! - Discovery: Resolves headless service DNS to get peer pod IPs
//! - Sync: On store, broadcasts to peers via HTTP POST
//! - Conflict: Uses timestamp-based last-write-wins
//! - Startup: Pulls state from peers on initialization

use reqwest::Client;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::types::StoreRequest;

/// Peer sync configuration
#[derive(Clone)]
pub struct PeerSyncConfig {
    /// Kubernetes headless service name (e.g., "freshcredit-blockchain-headless")
    pub headless_service: String,
    /// Namespace for the service
    pub namespace: String,
    /// Port for NOMT HTTP API
    pub port: u16,
    /// Current pod name (to exclude self from peers)
    pub pod_name: String,
    /// Enable peer sync
    pub enabled: bool,
}
// TAG: surface=blockchain owner=blockchain-team rule=BC-001

impl Default for PeerSyncConfig {
    fn default() -> Self {
        Self {
            headless_service: "freshcredit-blockchain-headless".to_string(),
            namespace: "freshcredit-blockchain".to_string(),
            port: 8081,
            pod_name: String::new(),
            enabled: false,
        }
    }
}

/// Peer sync service for NOMT sidecar
pub struct PeerSyncService {
    config: PeerSyncConfig,
    client: Client,
    /// Known peer addresses (cached)
    peers: Arc<RwLock<HashSet<String>>>,
}

impl PeerSyncService {
    /// Create a new peer sync service
    pub fn new(config: PeerSyncConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            client,
            peers: Arc::new(RwLock::new(HashSet::new())),
        }
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    }

    /// Discover peers via DNS lookup
    pub async fn discover_peers(&self) -> Vec<String> {
        if !self.config.enabled {
            return vec![];
        }

        // Kubernetes headless service DNS format:
        // <pod-name>.<headless-service>.<namespace>.svc.cluster.local
        // For StatefulSet: freshcredit-blockchain-0.freshcredit-blockchain-headless.freshcredit-blockchain.svc.cluster.local
        let dns_name = format!(
            "{}.{}.svc.cluster.local",
            self.config.headless_service, self.config.namespace
        );

        match tokio::net::lookup_host(format!("{}:{}", dns_name, self.config.port)).await {
            Ok(addrs) => {
                let peers: Vec<String> = addrs
                    .map(|addr| format!("http://{}:{}", addr.ip(), self.config.port))
                    .filter(|url| !url.contains(&self.config.pod_name))
                    .collect();

                // Update cached peers
                let mut cached = self.peers.write().await;
                // TAG: surface=api
                // TAG: surface=api
                *cached = peers.iter().cloned().collect();

                info!(count = peers.len(), "Discovered NOMT peers");
                peers
            }
            Err(e) => {
                warn!(error = %e, dns = %dns_name, "Failed to discover peers via DNS");
                vec![]
            } // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        }
    }

    /// Broadcast a store operation to all peers
    pub async fn broadcast_store(&self, request: &StoreRequest) {
        if !self.config.enabled {
            return;
        }

        let peers: Vec<String> = {
            let guard = self.peers.read().await;
            guard.iter().cloned().collect()
        };

        if peers.is_empty() {
            debug!("No peers to broadcast to");
            return;
        }

        for peer in peers {
            let url = format!("{peer}/sync/store");
            let client = self.client.clone();
            let req = request.clone();
            let peer_clone = peer.clone();

            // Fire and forget - don't block on peer responses
            tokio::spawn(async move {
                match client.post(&url).json(&req).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        debug!(peer = %peer_clone, "Broadcast to peer succeeded");
                    }
                    Ok(resp) => {
                        warn!(peer = %peer_clone, status = %resp.status(), "Peer rejected broadcast");
                    }
                    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
                    Err(e) => {
                        warn!(peer = %peer_clone, error = %e, "Failed to broadcast to peer");
                    }
                }
            });
        }
    }

    /// Check if peer sync is enabled
    pub const fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::StoreRequest;

    #[tokio::test]
    async fn test_peer_sync_disabled() {
        let config = PeerSyncConfig {
            enabled: false,
            ..Default::default()
        };
        let service = PeerSyncService::new(config);
        assert!(!service.is_enabled());
        let peers = service.discover_peers().await;
        assert!(peers.is_empty());
    }

    #[test]
    fn test_peer_sync_enabled() {
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        let config = PeerSyncConfig {
            enabled: true,
            ..Default::default()
        };
        let service = PeerSyncService::new(config);
        assert!(service.is_enabled());
    }

    #[tokio::test]
    async fn test_discover_peers_returns_empty_when_disabled() {
        let config = PeerSyncConfig {
            enabled: false,
            ..Default::default()
        };
        let service = PeerSyncService::new(config);
        let peers = service.discover_peers().await;
        assert!(peers.is_empty());
    }

    #[tokio::test]
    async fn test_broadcast_store_skips_when_disabled() {
        let config = PeerSyncConfig {
            enabled: false,
            ..Default::default()
        };
        let service = PeerSyncService::new(config);
        let request = StoreRequest {
            user_id: "user1".to_string(),
            report_type: "PlaidFinancial".to_string(),
            data: vec![1, 2, 3],
        };
        // Should not panic when peer sync is disabled
        service.broadcast_store(&request).await;
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
