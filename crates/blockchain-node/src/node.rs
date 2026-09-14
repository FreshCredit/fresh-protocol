//! Blockchain node setup and management.

use crate::cli::Cli;
use crate::config::{DEFAULT_RETRY_DELAY_SECS, HEALTH_CHECK_INTERVAL_SECS, MAX_RETRY_DELAY_SECS};
use blockchain_client::BlockchainClient;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[derive(Clone)]
pub struct AppState {
    /// Blockchain client - may be None during initialization or if connection fails
    pub client: Arc<RwLock<Option<BlockchainClient>>>,
    pub substrate_url: String,
    /// NOMT sidecar URL for unified verification (optional)
    pub nomt_sidecar_url: Option<String>,
    /// HTTP client for NOMT sidecar requests
    pub http_client: reqwest::Client,
    /// Connection status for health checks
    pub connection_status: Arc<RwLock<ConnectionStatus>>,
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
    Initializing,
    Connected,
    Disconnected,
    Error,
}

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    info!("Starting FreshCredit Blockchain Explorer");
    info!("Explorer UI: http://localhost:{}", cli.port);
    info!("Substrate node: {}", cli.substrate_url);
    if let Some(ref nomt_url) = cli.nomt_sidecar_url {
        info!("NOMT sidecar: {}", nomt_url);
    }

    // Initialize state with no client connection
    let connection_status = Arc::new(RwLock::new(ConnectionStatus::Initializing));
    let state = AppState {
        client: Arc::new(RwLock::new(None)),
        substrate_url: cli.substrate_url.clone(),
        nomt_sidecar_url: cli.nomt_sidecar_url,
        http_client: reqwest::Client::new(),
        connection_status: connection_status.clone(),
    };

    // Start HTTP server immediately - don't wait for Substrate connection
    let app = crate::rpc::create_app(state.clone());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", cli.port)).await?;
    info!("HTTP server listening on port {}", cli.port);
    // TAG: surface=blockchain owner=platform-team rule=GENERAL-001

    // Start blockchain client connection in background with retry logic
    let substrate_url = cli.substrate_url.clone();
    let client_arc = state.client.clone();
    tokio::spawn(async move {
        let mut retry_delay = DEFAULT_RETRY_DELAY_SECS;

        loop {
            info!(
                "Attempting to connect to Substrate node at {}...",
                substrate_url
            );
            match BlockchainClient::new(substrate_url.clone()).await {
                Ok(client) => {
                    info!("Successfully connected to Substrate node");

                    // Scope the write locks so they are released before we enter the
                    // long-running health-check loop. Holding them across awaits would
                    // block every HTTP handler that needs to read the client.
                    {
                        let mut client_guard = client_arc.write().await;
                        *client_guard = Some(client);
                    }
                    {
                        let mut status = connection_status.write().await;
                        *status = ConnectionStatus::Connected;
                    }

                    // Connection successful - keep the task alive but check periodically
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_secs(
                            HEALTH_CHECK_INTERVAL_SECS,
                        ))
                        .await;

                        // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
                        // Check if connection is still healthy. Clone the client so we
                        // don't hold the read lock across the health check RPC call.
                        let maybe_client = {
                            let client_guard = client_arc.read().await;
                            client_guard.clone()
                        };
                        if let Some(client) = maybe_client {
                            if let Err(e) = client.health_check().await {
                                warn!("Substrate connection health check failed: {}", e);
                                break; // Break inner loop to trigger reconnection
                            }
                        }
                    }

                    // Connection lost - mark as disconnected and retry
                    let mut status = connection_status.write().await;
                    *status = ConnectionStatus::Disconnected;
                }
                Err(e) => {
                    warn!(
                        "Failed to connect to Substrate node (retrying in {}s): {}",
                        retry_delay, e
                    );
                    let mut status = connection_status.write().await;
                    *status = ConnectionStatus::Error;
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(retry_delay)).await;
            retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY_SECS);
        }
    });

    info!("Explorer started successfully - serving traffic while connecting to blockchain");

    // P0-BEST-PRACTICES: Graceful shutdown for zero-downtime deployments
    axum::serve(listener, app)
        .with_graceful_shutdown(crate::helpers::shutdown_signal())
        .await?;

    info!("👋 Blockchain explorer stopped gracefully");
    Ok(())
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Try to reconnect the client by creating a fresh connection
pub async fn try_reconnect(state: &AppState) -> Result<(), String> {
    info!(
        "Connection lost, creating fresh connection to {}",
        state.substrate_url
    );
    match BlockchainClient::new(state.substrate_url.clone()).await {
        Ok(new_client) => {
            let mut client = state.client.write().await;
            *client = Some(new_client);
            drop(client);

            let mut status = state.connection_status.write().await;
            *status = ConnectionStatus::Connected;

            info!("Successfully created fresh connection to blockchain node");
            Ok(())
        }
        Err(e) => {
            error!("Failed to create fresh connection: {}", e);
            let mut status = state.connection_status.write().await;
            *status = ConnectionStatus::Error;
            Err(format!("Failed to reconnect: {e}"))
        }
    }
}
