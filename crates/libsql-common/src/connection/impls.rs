use async_trait::async_trait;
use std::fmt;
use std::path::PathBuf;

use super::types::*;

// TAG: surface=database owner=platform-team rule=DB-001
impl fmt::Display for ConnectionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectRemote => write!(f, "direct-remote"),
            #[cfg(feature = "embedded-replica")]
            Self::EmbeddedReplica => write!(f, "embedded-replica"),
            Self::LocalOnly => write!(f, "local-only"),
            #[cfg(feature = "embedded-replica")]
            Self::Adaptive => write!(f, "adaptive"),
        }
    }
}

impl std::str::FromStr for ConnectionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "remote" | "direct-remote" => Ok(Self::DirectRemote),
            "local" | "local-only" => Ok(Self::LocalOnly),

            #[cfg(feature = "embedded-replica")]
            "replica" | "embedded-replica" => Ok(Self::EmbeddedReplica),

            #[cfg(feature = "embedded-replica")]
            "adaptive" => Ok(Self::Adaptive),

            #[cfg(not(feature = "embedded-replica"))]
            "replica" | "embedded-replica" | "adaptive" => Err(format!(
                "Connection mode '{}' requires the 'embedded-replica' feature. \
                 Enable it in Cargo.toml: freshcredit-libsql-common = {{ features = [\"embedded-replica\"] }}",
                s
            )),

            _ => Err(format!("Unknown connection mode: {s}")),
        }
    }
}

// TAG: surface=database owner=platform-team rule=DB-001
#[cfg(feature = "embedded-replica")]
impl fmt::Display for ReadConsistency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eventual => write!(f, "eventual"),
            Self::Strong => write!(f, "strong"),
            Self::Adaptive => write!(f, "adaptive"),
        }
    }
}

// TAG: surface=database owner=platform-team rule=DB-001
#[cfg(feature = "embedded-replica")]
impl std::str::FromStr for ReadConsistency {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "eventual" => Ok(Self::Eventual),
            "strong" => Ok(Self::Strong),
            "adaptive" => Ok(Self::Adaptive),
            _ => Err(format!("Unknown read consistency: {s}")),
        }
    }
}

impl ConnectionHealth {
    /// Create a healthy status
    #[must_use]
    pub const fn healthy(mode: ConnectionMode, latency_ms: u64) -> Self {
        Self {
            mode,
            latency_ms,
            is_healthy: true,
            last_sync_at: None,
            cache_hit_rate: None,
        }
    }

    /// Create an unhealthy status
    #[must_use]
    pub const fn unhealthy(mode: ConnectionMode, latency_ms: u64) -> Self {
        Self {
            mode,
            latency_ms,
            is_healthy: false,
            last_sync_at: None,
            cache_hit_rate: None,
        }
    }
}

// TAG: surface=database owner=platform-team rule=DB-001
impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            mode: ConnectionMode::DirectRemote,
            remote_url: String::new(),
            auth_token: String::new(),
            local_path: None,
            enable_fallback: false,
            sync_interval_secs: Some(60),
            #[cfg(feature = "embedded-replica")]
            read_consistency: ReadConsistency::Eventual,
            timeout_secs: 30,
            max_retries: 3,
        }
    }
}

impl ConnectionConfig {
    /// Create configuration from environment variables
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn from_env() -> anyhow::Result<Self> {
        use std::env;

        let mode = env::var("LIBSQL_MODE")
            .unwrap_or_else(|_| "remote".to_string())
            // TAG: surface=database owner=platform-team rule=GENERAL-001
            .parse::<ConnectionMode>()
            .map_err(|e| anyhow::anyhow!(e))?;

        let remote_url = env::var("TURSO_URL")
            .or_else(|_| env::var("LIBSQL_URL"))
            .unwrap_or_default();

        let auth_token = env::var("TURSO_AUTH_TOKEN")
            .or_else(|_| env::var("LIBSQL_AUTH_TOKEN"))
            .unwrap_or_default();

        let local_path = env::var("LIBSQL_LOCAL_PATH").ok().map(PathBuf::from);

        let enable_fallback = env::var("LIBSQL_ENABLE_FALLBACK")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let sync_interval_secs = env::var("LIBSQL_SYNC_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok());

        #[cfg(feature = "embedded-replica")]
        let read_consistency = env::var("LIBSQL_READ_CONSISTENCY")
            .unwrap_or_else(|_| "eventual".to_string())
            .parse::<ReadConsistency>()
            .map_err(|e| anyhow::anyhow!(e))?;

        // TAG: surface=database owner=platform-team rule=DB-001
        let timeout_secs = env::var("LIBSQL_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30);

        let max_retries = env::var("LIBSQL_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(3);

        Ok(Self {
            mode,
            remote_url,
            auth_token,
            local_path,
            enable_fallback,
            sync_interval_secs,
            #[cfg(feature = "embedded-replica")]
            read_consistency,
            timeout_secs,
            max_retries,
        })
    }

    // TAG: surface=database owner=platform-team rule=DB-001
    /// Validate the configuration
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn validate(&self) -> anyhow::Result<()> {
        match self.mode {
            ConnectionMode::DirectRemote => {
                if self.remote_url.is_empty() {
                    return Err(anyhow::anyhow!(
                        "Remote URL is required for direct-remote mode"
                    ));
                }
            }
            ConnectionMode::LocalOnly => {
                if self.local_path.is_none() {
                    return Err(anyhow::anyhow!(
                        "Local path is required for local-only mode"
                    ));
                }
            }
            #[cfg(feature = "embedded-replica")]
            ConnectionMode::EmbeddedReplica => {
                if self.remote_url.is_empty() {
                    return Err(anyhow::anyhow!(
                        "Remote URL is required for embedded replica mode"
                    ));
                }
                if self.local_path.is_none() {
                    return Err(anyhow::anyhow!(
                        "Local path is required for embedded replica mode"
                    ));
                }
            }
            #[cfg(feature = "embedded-replica")]
            ConnectionMode::Adaptive => {
                if self.remote_url.is_empty() {
                    return Err(anyhow::anyhow!("Remote URL is required for adaptive mode"));
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl<T: DatabaseConnection> DatabaseConnectionExt for T {}
