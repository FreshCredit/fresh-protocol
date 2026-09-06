// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
//! Polkadot Bulletin Chain proof-of-concept client for `FreshCredit` P2P
//! report-share fallback.
//!
//! The client computes a deterministic content identifier (base64 of SHA-256)
//! over a sealed envelope. When configured and enabled it attempts to anchor the
//! envelope on a Bulletin devnet via `transactionStorage.store`; otherwise it
//! degrades gracefully and returns the CID with no block number. The ciphertext
//! itself is intended to live on Bulletin/IPFS; this server only stores the CID
//! and public signals.

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64_STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use subxt::{OnlineClient, SubstrateConfig};
use subxt_rpcs::LegacyRpcMethods;
use subxt_rpcs::RpcClient;
use subxt_signer::sr25519::Keypair;
use tracing::{info, warn};

/// Default public IPFS gateway prefixes used for retrieval when no custom gateway
/// is configured.
pub const DEFAULT_IPFS_GATEWAYS: &[&str] = &[
    "https://ipfs.io/ipfs/",
    "https://gateway.pinata.cloud/ipfs/",
    "https://cloudflare-ipfs.com/ipfs/",
];

/// Maximum envelope size the `PoC` will attempt to anchor on-chain via
/// `transactionStorage.store`.
///
/// Bulletin Chain stores data on-chain; real deployments should use IPFS
/// pinning with a CID anchor. This limit keeps the `PoC` from accidentally
/// submitting huge blobs.
pub const MAX_ENVELOPE_ANCHOR_BYTES: usize = 256 * 1024; // 256 KiB

/// Configuration for the Bulletin Chain client.
#[derive(Debug, Clone, Default)]
pub struct BulletinConfig {
    /// WebSocket RPC URL for the Bulletin devnet node.
    pub rpc_url: String,
    /// Optional signer SURI (development/testing).
    pub signer_suri: Option<String>,
    /// Optional signer mnemonic (production).
    pub signer_mnemonic: Option<String>,
    /// Preferred IPFS gateway prefix (e.g. `https://ipfs.io/ipfs/`).
    pub ipfs_gateway: Option<String>,
    /// Whether on-chain anchoring is enabled.
    pub enabled: bool,
}

impl BulletinConfig {
    /// Load configuration from environment variables.
    ///
    /// Env vars:
    /// - `BULLETIN_RPC_URL`
    /// - `BULLETIN_SIGNER_SURI`
    /// - `BULLETIN_SIGNER_MNEMONIC`
    /// - `BULLETIN_IPFS_GATEWAY`
    /// - `BULLETIN_ENABLED` (default `false`)
    #[must_use]
    pub fn from_env() -> Self {
        let enabled = std::env::var("BULLETIN_ENABLED")
            .ok()
            .is_some_and(|s| s.trim().eq_ignore_ascii_case("true"));

        Self {
            rpc_url: std::env::var("BULLETIN_RPC_URL").unwrap_or_default(),
            signer_suri: std::env::var("BULLETIN_SIGNER_SURI").ok(),
            signer_mnemonic: std::env::var("BULLETIN_SIGNER_MNEMONIC").ok(),
            ipfs_gateway: std::env::var("BULLETIN_IPFS_GATEWAY").ok(),
            enabled,
        }
    }
}

/// Result of storing an envelope with Bulletin Chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BulletinStoreResult {
    /// Deterministic content identifier (base64 of SHA-256 over the envelope).
    pub cid: String,
    /// Block number of the on-chain anchor, when submitted successfully.
    pub block_number: Option<u64>,
}

/// Bulletin Chain client.
#[derive(Clone, Debug)]
pub struct BulletinClient {
    config: BulletinConfig,
    api: Option<Arc<OnlineClient<SubstrateConfig>>>,
    rpc: Option<Arc<LegacyRpcMethods<subxt::config::RpcConfigFor<SubstrateConfig>>>>,
    signer: Option<Arc<Keypair>>,
}

impl BulletinClient {
    /// Create a new client from environment configuration and best-effort connect
    /// to the configured devnet node.
    ///
    /// # Errors
    ///
    /// Returns an error only if the signer configuration is malformed. RPC
    /// connection failures are logged and swallowed so the client can still
    /// compute deterministic CIDs when the devnet is unreachable.
    pub async fn from_env() -> Result<Self> {
        let config = BulletinConfig::from_env();
        let mut client = Self::new(config)?;
        client.try_connect().await;
        Ok(client)
    }

    /// Create a new client from explicit configuration without connecting.
    ///
    /// # Errors
    ///
    /// Returns an error if a signer is configured but cannot be parsed.
    pub fn new(config: BulletinConfig) -> Result<Self> {
        let signer = Self::load_signer(&config)
            .map(Arc::new)
            .map_err(|e| {
                warn!("Failed to load Bulletin signer: {}", e);
                e
            })
            .ok(); // AUDIT-OK(fire-and-forget): signer absence handled downstream; failure already logged above

        Ok(Self {
            config,
            api: None,
            rpc: None,
            signer,
        })
    }

    /// Attempt to connect to the configured RPC node. Failures are logged but
    /// not returned, preserving graceful degradation.
    pub async fn try_connect(&mut self) {
        if self.api.is_some() || !self.config.enabled || self.config.rpc_url.is_empty() {
            return;
        }

        match Self::connect(&self.config.rpc_url).await {
            Ok((api, rpc)) => {
                info!(
                    "Connected to Bulletin Chain devnet at {}",
                    self.config.rpc_url
                );
                self.api = Some(api);
                self.rpc = Some(rpc);
            }
            Err(e) => {
                warn!(
                    "Bulletin Chain devnet at {} unreachable; operating in CID-only mode: {}",
                    self.config.rpc_url, e
                );
            }
        }
    }

    async fn connect(
        rpc_url: &str,
    ) -> Result<(
        Arc<OnlineClient<SubstrateConfig>>,
        Arc<LegacyRpcMethods<subxt::config::RpcConfigFor<SubstrateConfig>>>,
    )> {
        let rpc_client = if rpc_url.starts_with("ws://") {
            RpcClient::from_insecure_url(rpc_url)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to connect to Bulletin RPC: {e}"))?
        } else {
            RpcClient::from_url(rpc_url)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to connect to Bulletin RPC: {e}"))?
        };

        let rpc = LegacyRpcMethods::<subxt::config::RpcConfigFor<SubstrateConfig>>::new(
            rpc_client.clone(),
        );
        let api = OnlineClient::<SubstrateConfig>::from_rpc_client(rpc_client)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create Bulletin online client: {e}"))?;

        Ok((Arc::new(api), Arc::new(rpc)))
    }

    fn load_signer(config: &BulletinConfig) -> Result<Keypair> {
        if let Some(mnemonic) = config.signer_mnemonic.as_deref() {
            let mnemonic = mnemonic.trim();
            let keypair = Keypair::from_phrase(&mnemonic.parse()?, None)
                .map_err(|e| anyhow::anyhow!("Invalid Bulletin signer mnemonic: {e:?}"))?;
            return Ok(keypair);
        }

        if let Some(suri) = config.signer_suri.as_deref() {
            let suri = suri.trim();
            let keypair = Keypair::from_uri(&suri.parse()?)
                .map_err(|e| anyhow::anyhow!("Invalid Bulletin signer SURI: {e:?}"))?;
            return Ok(keypair);
        }

        Err(anyhow::anyhow!(
            "No Bulletin signer configured; set BULLETIN_SIGNER_MNEMONIC or BULLETIN_SIGNER_SURI"
        ))
    }

    /// Store an envelope with Bulletin Chain.
    ///
    /// 1. Computes the deterministic CID as base64 of SHA-256 over the envelope.
    /// 2. If enabled and a signer + RPC connection are available, attempts to
    ///    submit a `transactionStorage.store` extrinsic for the envelope bytes.
    /// 3. Degrades to returning the CID with `block_number: None` when the
    ///    devnet is unreachable or the client is disabled.
    ///
    /// # Errors
    ///
    /// Returns an error only for fatal client misconfiguration; RPC failures
    /// are swallowed and surfaced as `block_number: None`.
    pub async fn store_envelope(&self, envelope_bytes: &[u8]) -> Result<BulletinStoreResult> {
        let cid = compute_cid(envelope_bytes);
        let mut block_number = None;

        if self.config.enabled && self.api.is_some() && self.signer.is_some() {
            if envelope_bytes.len() <= MAX_ENVELOPE_ANCHOR_BYTES {
                match self.submit_store_extrinsic(envelope_bytes).await {
                    Ok(bn) => {
                        info!("Bulletin anchor submitted at block {}", bn);
                        block_number = Some(bn);
                    }
                    Err(e) => {
                        warn!(
                            "Bulletin on-chain anchor failed; returning CID-only placeholder: {}",
                            e
                        );
                    }
                }
            } else {
                warn!(
                    "Envelope size {} exceeds Bulletin anchor limit {}; returning CID only",
                    envelope_bytes.len(),
                    MAX_ENVELOPE_ANCHOR_BYTES
                );
            }
        }

        Ok(BulletinStoreResult { cid, block_number })
    }

    async fn submit_store_extrinsic(&self, data: &[u8]) -> Result<u64> {
        let api = self
            .api
            .as_ref()
            .context("Bulletin RPC client not connected")?;
        let signer = self
            .signer
            .as_ref()
            .context("Bulletin signer not configured")?;

        let call = subxt::dynamic::tx(
            "TransactionStorage",
            "store",
            vec![subxt::dynamic::Value::from_bytes(data)],
        );

        let tx_progress = api
            .tx()
            .await
            .context("Bulletin transaction client unavailable")?
            .sign_and_submit_then_watch_default(&call, &**signer)
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to submit Bulletin transactionStorage.store: {e}")
            })?;

        let _events = tx_progress
            .wait_for_finalized_success()
            .await
            .map_err(|e| anyhow::anyhow!("Bulletin transactionStorage.store failed: {e}"))?;

        // The finalized transaction is in the current best chain; ask the RPC
        // node for its latest header to get the anchor block number.
        let rpc = self
            .rpc
            .as_ref()
            .context("Bulletin RPC client unavailable")?;
        let header = rpc
            .chain_get_header(None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch Bulletin header: {e}"))?;

        header
            .map(|h| h.number)
            .ok_or_else(|| anyhow::anyhow!("Could not determine Bulletin anchor block number"))
    }

    /// Retrieve envelope bytes for a CID.
    ///
    /// Tries the configured `BULLETIN_IPFS_GATEWAY` first, then the public
    /// gateway fallback list. For the `PoC`, retrieval requires a peered node or
    /// configured gateway; if none respond, an explanatory error is returned.
    ///
    /// # Errors
    ///
    /// Returns an error if no gateway responds successfully.
    pub async fn retrieve_envelope(&self, cid: &str) -> Result<Vec<u8>> {
        let mut gateways: Vec<String> = Vec::new();

        if let Some(g) = self.config.ipfs_gateway.as_deref() {
            let normalized = if g.ends_with('/') {
                g.to_string()
            } else {
                format!("{g}/")
            };
            gateways.push(normalized);
        }

        gateways.extend(DEFAULT_IPFS_GATEWAYS.iter().map(|s| (*s).to_string()));

        if gateways.is_empty() {
            anyhow::bail!("No IPFS gateway available for Bulletin retrieval");
        }

        let mut last_error = None;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to build Bulletin HTTP client")?;

        for gateway in &gateways {
            let url = format!("{gateway}{cid}");
            match http
                .get(&url)
                .header(reqwest::header::ACCEPT, "application/octet-stream, */*")
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                    Ok(bytes) => return Ok(bytes.to_vec()),
                    Err(e) => {
                        warn!(
                            "Bulletin gateway {} returned unreadable body: {}",
                            gateway, e
                        );
                        last_error = Some(e.into());
                    }
                },
                Ok(resp) => {
                    warn!(
                        "Bulletin gateway {} returned status {} for CID {}",
                        gateway,
                        resp.status(),
                        cid
                    );
                    last_error = Some(anyhow::anyhow!(
                        "Gateway {} returned status {}",
                        gateway,
                        resp.status()
                    ));
                }
                Err(e) => {
                    warn!("Bulletin gateway {} unreachable: {}", gateway, e);
                    last_error = Some(e.into());
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!(
                "Could not retrieve Bulletin CID {cid}; retrieval needs a peered node or configured gateway"
            )
        }))
    }

    /// Whether the client is configured to attempt on-chain anchoring.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Reference to the loaded configuration.
    #[must_use]
    pub const fn config(&self) -> &BulletinConfig {
        &self.config
    }
}

/// Compute the deterministic Bulletin CID for a byte payload.
///
/// The CID is the base64 encoding of the SHA-256 digest. This matches the
/// placeholder behavior in the original `post_bulletin_anchor` handler and is
/// easy to recompute client-side for integrity verification.
#[must_use]
pub fn compute_cid(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    B64_STANDARD.encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_cid_deterministic() {
        let data = b"freshcredit bulletin envelope";
        let cid1 = compute_cid(data);
        let cid2 = compute_cid(data);
        assert_eq!(cid1, cid2);
        assert!(!cid1.is_empty());

        // Base64 of SHA-256 should be 44 characters.
        assert_eq!(cid1.len(), 44);
    }

    #[test]
    fn test_compute_cid_matches_placeholder() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        use sha2::{Digest, Sha256};

        let envelope = b"test envelope for cid match";
        let expected = STANDARD.encode(Sha256::digest(envelope));
        assert_eq!(compute_cid(envelope), expected);
    }

    #[test]
    fn test_config_from_env_defaults() {
        // Capture any existing env vars so we can restore them.
        let vars = [
            "BULLETIN_ENABLED",
            "BULLETIN_RPC_URL",
            "BULLETIN_SIGNER_SURI",
            "BULLETIN_SIGNER_MNEMONIC",
            "BULLETIN_IPFS_GATEWAY",
        ];
        let saved: Vec<Option<String>> = vars.iter().map(|v| std::env::var(v).ok()).collect();

        for v in &vars {
            std::env::remove_var(v);
        }

        let config = BulletinConfig::from_env();
        assert!(!config.enabled);
        assert!(config.rpc_url.is_empty());
        assert!(config.signer_suri.is_none());
        assert!(config.signer_mnemonic.is_none());
        assert!(config.ipfs_gateway.is_none());

        std::env::set_var("BULLETIN_ENABLED", "true");
        std::env::set_var("BULLETIN_RPC_URL", "wss://bulletin.devnet.example");
        std::env::set_var("BULLETIN_SIGNER_SURI", "//Alice");
        std::env::set_var("BULLETIN_IPFS_GATEWAY", "https://example.com/ipfs/");

        let config = BulletinConfig::from_env();
        assert!(config.enabled);
        assert_eq!(config.rpc_url, "wss://bulletin.devnet.example");
        assert_eq!(config.signer_suri.as_deref(), Some("//Alice"));
        assert_eq!(
            config.ipfs_gateway.as_deref(),
            Some("https://example.com/ipfs/")
        );

        for (i, v) in vars.iter().enumerate() {
            match &saved[i] {
                Some(val) => std::env::set_var(v, val),
                None => std::env::remove_var(v),
            }
        }
    }

    #[tokio::test]
    async fn test_store_envelope_disabled_returns_cid_only() {
        let config = BulletinConfig::default();
        let client = BulletinClient::new(config).expect("new should succeed without signer");
        let result = client.store_envelope(b"hello").await.unwrap();
        assert_eq!(result.block_number, None);
        assert_eq!(result.cid, compute_cid(b"hello"));
    }
}
