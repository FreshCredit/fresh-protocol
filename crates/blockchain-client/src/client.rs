//! Blockchain client for Substrate interaction

use anyhow::Result;
use freshcredit_config::Environment;
use std::sync::Arc;
use subxt::{OnlineClient, SubstrateConfig};
use subxt_rpcs::LegacyRpcMethods;
use subxt_rpcs::RpcClient;
use subxt_signer::sr25519::Keypair;
use tracing::{error, info, warn};

pub mod block;
pub mod hash;

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// `FreshCredit` Substrate blockchain client
#[derive(Clone, Debug)]
pub struct BlockchainClient {
    pub(crate) api: Arc<OnlineClient<SubstrateConfig>>,
    pub(crate) rpc: Arc<LegacyRpcMethods<subxt::config::RpcConfigFor<SubstrateConfig>>>,
    pub(crate) signer: Arc<Keypair>,
    pub(crate) rpc_url: String,
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
impl BlockchainClient {
    /// Create a new blockchain client with async initialization
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn new(rpc_url: String) -> Result<Self> {
        // Create RPC client first
        // Use from_insecure_url for ws:// URLs (internal VPC connections)
        // Use from_url for wss:// URLs (secure external connections)
        let rpc_client = if rpc_url.starts_with("ws://") {
            RpcClient::from_insecure_url(&rpc_url)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to connect to RPC: {e}"))?
        } else {
            RpcClient::from_url(&rpc_url)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to connect to RPC: {e}"))?
        };

        // Create legacy RPC methods for system calls
        let rpc = LegacyRpcMethods::<subxt::config::RpcConfigFor<SubstrateConfig>>::new(
            rpc_client.clone(),
        );

        // Create online client from RPC
        let api = OnlineClient::<SubstrateConfig>::from_rpc_client(rpc_client)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create online client: {e}"))?;

        // Load signer from environment
        let signer = Self::load_signer()?;
        // TAG: surface=blockchain owner=platform-team rule=GENERAL-001

        info!("Connected to Substrate node at {}", rpc_url);

        Ok(Self {
            api: Arc::new(api),
            rpc: Arc::new(rpc),
            signer: Arc::new(signer),
            rpc_url,
        })
    }

    /// Load the service account signer from environment
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub(crate) fn load_signer() -> Result<Keypair> {
        // Try mnemonic first (production)
        if let Ok(mnemonic) = std::env::var("BLOCKCHAIN_SIGNER_MNEMONIC") {
            let mnemonic = mnemonic.trim();
            let keypair = Keypair::from_phrase(&mnemonic.parse()?, None)
                .map_err(|e| anyhow::anyhow!("Invalid mnemonic: {e:?}"))?;
            info!("Loaded signer from mnemonic");
            return Ok(keypair);
        }

        // Try SURI (development/testing)
        if let Ok(suri) = std::env::var("BLOCKCHAIN_SIGNER_SURI") {
            let suri = suri.trim();
            let keypair = Keypair::from_uri(&suri.parse()?)
                .map_err(|e| anyhow::anyhow!("Invalid SURI: {e:?}"))?;
            info!("Loaded signer from SURI");
            return Ok(keypair);
        }

        // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
        // SECURITY: No hardcoded fallback in production
        // CRITICAL-001: Removed //Alice default to prevent key compromise
        if Environment::detect().is_production() {
            error!(
                "BLOCKCHAIN_SIGNER_MNEMONIC or BLOCKCHAIN_SIGNER_SURI must be set in production"
            );
            return Err(anyhow::anyhow!(
                "Missing required signer configuration: BLOCKCHAIN_SIGNER_MNEMONIC or BLOCKCHAIN_SIGNER_SURI"
            ));
        }

        // Development-only fallback with explicit warning
        warn!("SECURITY: Using development signer //Alice - NEVER use in production!");
        warn!("Set BLOCKCHAIN_SIGNER_MNEMONIC or BLOCKCHAIN_SIGNER_SURI for production");
        let keypair = Keypair::from_uri(&"//Alice".parse()?)
            .map_err(|e| anyhow::anyhow!("Failed to create development keypair: {e:?}"))?;
        Ok(keypair)
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Create a new blockchain client from environment variables
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn from_env() -> Result<Self> {
        // HARDCODED_URL: Default blockchain WebSocket endpoints - override with BLOCKCHAIN_RPC_URL env var
        // HARDCODED_URL: Default blockchain WebSocket URLs for local and production
        let rpc_url = std::env::var("BLOCKCHAIN_RPC_URL")
            .or_else(|_| std::env::var("BLOCKCHAIN_URL"))
            .unwrap_or_else(|_| {
                // Use runtime environment detection, not compile-time cfg!(debug_assertions)
                // cfg!(debug_assertions) is false in --release builds even when running locally
                if Environment::detect().is_production() {
                    "wss://blockchain.freshcredit.com".to_string()
                } else {
                    "ws://localhost:9944".to_string()
                }
            });

        info!("Initializing blockchain client with URL: {}", rpc_url);
        Self::new(rpc_url).await
    }

    /// Get the RPC URL this client is connected to
    #[must_use]
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Reconnect to the blockchain node (creates a new client with the same URL)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn reconnect(&self) -> Result<Self> {
        info!("Reconnecting to Substrate node at {}", self.rpc_url);
        Self::new(self.rpc_url.clone()).await
    }

    /// Check if the connection is still alive
    pub async fn is_connected(&self) -> bool {
        self.rpc.system_health().await.is_ok()
    }

    /// Check if the blockchain service is healthy
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn health_check(&self) -> Result<bool> {
        match self.rpc.system_health().await {
            Ok(health) => {
                let is_healthy = !health.is_syncing;
                if is_healthy {
                    info!(
                        "Blockchain health check passed: {} peers, syncing={}",
                        // TAG: surface=api
                        // TAG: surface=api
                        health.peers,
                        health.is_syncing // TAG: surface=blockchain owner=platform-team rule=GENERAL-001
                    );
                } else {
                    warn!("Blockchain is still syncing");
                }
                Ok(is_healthy)
            }
            Err(e) => {
                // Surface RPC failures as Err so callers treat them as a dead
                // connection: the watchdog in apps/blockchain breaks its inner
                // loop on Err and reconnects, and the HTTP handlers run their
                // reconnect-and-retry path. Returning Ok(false) here previously
                // read as merely "syncing", so a dead WS (e.g. subxt's
                // "restart required") kept the stale client in service until a
                // pod restart.
                error!("Blockchain health check failed: {}", e);
                Err(anyhow::anyhow!("Blockchain health check failed: {e}"))
            }
        }
    }

    /// Sanitize `user_id` for logging - returns truncated/hashed version
    pub(crate) fn sanitize_user_id_for_logs(user_id: &str) -> String {
        if user_id.len() <= 8 {
            "***".to_string()
        } else {
            format!("{}...{}", &user_id[..4], &user_id[user_id.len() - 4..])
        }
    }

    /// Get the signer's account ID (service account under which hashes are stored)
    #[must_use]
    pub fn get_signer_account_id(&self) -> subxt::utils::AccountId32 {
        subxt::utils::AccountId32::from(self.signer.public_key().0)
    }
}
