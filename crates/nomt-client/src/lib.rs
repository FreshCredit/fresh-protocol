// TAG: surface=blockchain owner=blockchain-team rule=BC-001
//! NOMT Client - HTTP client for NOMT server API
//!
//! This crate provides a client for interacting with the NOMT HTTP API,
//! used by the blockchain client and other services that need to store
//! and verify hashes.

#![warn(missing_docs)]
#![deny(unsafe_code)]

use freshcredit_nomt_core::{
    HashMetadata, NomtProof, StorageStats, StoreRequest, StoreResponse, VerifyRequest,
    VerifyResponse,
};
use reqwest::{Client, Url};
use std::time::Duration;
use thiserror::Error;

/// Errors that can occur in NOMT client operations
#[derive(Debug, Error)]
pub enum ClientError {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    /// Invalid URL
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    /// API error
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    #[error("API error: {0}")]
    Api(String),
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// URL parse error
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
    /// Timeout
    #[error("Request timeout")]
    Timeout,
}

/// NOMT HTTP client
#[derive(Clone)]
pub struct NomtClient {
    http: Client,
    base_url: Url,
    api_key: Option<String>,
}

impl std::fmt::Debug for NomtClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NomtClient")
            .field("base_url", &self.base_url.as_str())
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .finish_non_exhaustive()
    }
}

impl NomtClient {
    /// Create a new NOMT client
    ///
    /// # Arguments
    /// * `base_url` - The base URL of the NOMT server (e.g., "<http://localhost:8080>")
    ///
    /// # Example
    /// ```rust,no_run
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    /// use nomt_client::NomtClient;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = NomtClient::new("http://localhost:8080")?;
    /// # Ok(())
    /// # }
    /// ```
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn new(base_url: impl AsRef<str>) -> Result<Self, ClientError> {
        let base_url =
            Url::parse(base_url.as_ref()).map_err(|e| ClientError::InvalidUrl(e.to_string()))?;

        let http = Client::builder().timeout(Duration::from_secs(30)).build()?;
        let api_key = std::env::var("NOMT_SIDECAR_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());

        Ok(Self {
            http,
            base_url,
            api_key,
        })
    }

    /// Create a new client with custom timeout
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn with_timeout(base_url: impl AsRef<str>, timeout_secs: u64) -> Result<Self, ClientError> {
        let base_url =
            Url::parse(base_url.as_ref()).map_err(|e| ClientError::InvalidUrl(e.to_string()))?;

        let http = Client::builder()
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            .timeout(Duration::from_secs(timeout_secs))
            .build()?;
        let api_key = std::env::var("NOMT_SIDECAR_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());

        Ok(Self {
            http,
            base_url,
            api_key,
        })
    }

    /// Attach an explicit `x-api-key` for authenticated sidecar/server deployments.
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Store data and get a proof
    ///
    /// # Arguments
    /// * `data` - The data to store
    /// * `metadata` - Optional metadata for the hash
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn store(
        &self,
        data: impl Into<String>,
        metadata: Option<HashMetadata>,
    ) -> Result<StoreResponse, ClientError> {
        let request = StoreRequest {
            data: data.into(),
            metadata,
        };

        let url = self.base_url.join("/store")?;
        let mut req_builder = self.http.post(url).json(&request);
        if let Some(ref key) = self.api_key {
            if !key.is_empty() {
                req_builder = req_builder.header("x-api-key", key);
            }
        }
        let response = req_builder.send().await?;

        if response.status().is_success() {
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            Ok(response.json().await?)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(ClientError::Api(format!("Store failed: {error_text}")))
        }
    }

    /// Verify a hash
    ///
    /// # Arguments
    /// * `hash` - The hash to verify
    /// * `expected_root` - Optional expected root hash
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn verify(
        &self,
        hash: impl Into<String>,
        expected_root: Option<String>,
    ) -> Result<VerifyResponse, ClientError> {
        let request = VerifyRequest {
            hash: hash.into(),
            expected_root,
        };

        let url = self.base_url.join("/verify")?;
        let mut req_builder = self.http.post(url).json(&request);
        if let Some(ref key) = self.api_key {
            if !key.is_empty() {
                req_builder = req_builder.header("x-api-key", key);
            }
        }
        let response = req_builder.send().await?;
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(ClientError::Api(format!("Verify failed: {error_text}")))
        }
    }

    /// Verify a hash against the `nomt-sidecar` `GET /verify/:hash` contract.
    /// # Errors
    ///
    /// Returns an error if the request fails or the sidecar returns a non-success status.
    pub async fn verify_sidecar_hash(&self, hash: &str) -> Result<serde_json::Value, ClientError> {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|()| ClientError::InvalidUrl(self.base_url.to_string()))?
            .extend(["verify", hash]);
        let mut req_builder = self.http.get(url);
        if let Some(ref key) = self.api_key {
            if !key.is_empty() {
                req_builder = req_builder.header("x-api-key", key);
            }
        }
        let response = req_builder.send().await?;
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(ClientError::Api(format!(
                "Sidecar verify failed: {error_text}"
            )))
        }
    }

    /// Fetch a Merkle inclusion proof from the `nomt-sidecar` `GET /proof/:hash` contract.
    /// # Errors
    ///
    /// Returns an error if the request fails or no proof is found.
    pub async fn get_sidecar_proof(&self, hash: &str) -> Result<NomtProof, ClientError> {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|()| ClientError::InvalidUrl(self.base_url.to_string()))?
            .extend(["proof", hash]);
        let mut req_builder = self.http.get(url);
        if let Some(ref key) = self.api_key {
            if !key.is_empty() {
                req_builder = req_builder.header("x-api-key", key);
            }
        }
        let response = req_builder.send().await?;
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(ClientError::Api(format!(
                "Sidecar proof failed: {error_text}"
            )))
        }
    }

    /// Get storage statistics
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn stats(&self) -> Result<StorageStats, ClientError> {
        let url = self.base_url.join("/stats")?;
        let mut req_builder = self.http.get(url);
        if let Some(ref key) = self.api_key {
            if !key.is_empty() {
                req_builder = req_builder.header("x-api-key", key);
            }
        }
        let response = req_builder.send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(ClientError::Api(format!("Stats failed: {error_text}")))
        }
    }

    /// Get the current Merkle root
    /// # Errors
    ///
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    /// Returns an error if the operation fails.
    pub async fn current_root(&self) -> Result<String, ClientError> {
        let stats = self.stats().await?;
        Ok(stats.current_root)
    }

    /// Anchor a hash to the blockchain
    ///
    /// This stores the hash and returns a proof that can be anchored.
    /// The anchoring itself is done by the caller using the blockchain client.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn prepare_anchor(
        &self,
        data: impl Into<String>,
        metadata: Option<HashMetadata>,
    ) -> Result<NomtProof, ClientError> {
        let response = self.store(data, metadata).await?;

        // Get the proof for the stored hash
        let verify_response = self.verify(response.hash, Some(response.root)).await?;

        verify_response
            .proof
            .ok_or_else(|| ClientError::Api("No proof returned".to_string()))
    }
}

// TAG: surface=blockchain owner=blockchain-team rule=BC-001

/// Result type for client operations
pub type ClientResult<T> = Result<T, ClientError>;

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{body_json, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[test]
    fn test_client_creation() {
        let client = NomtClient::new("http://localhost:8080");
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_with_timeout() {
        let client = NomtClient::with_timeout("http://localhost:8080", 5);
        assert!(client.is_ok());
    }

    #[test]
    fn test_invalid_url() {
        let client = NomtClient::new("not-a-valid-url");
        assert!(client.is_err());
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
        assert!(client.unwrap_err().to_string().contains("Invalid URL"));
    }

    #[test]
    fn test_client_error_display_variants() {
        let err = ClientError::InvalidUrl("bad".to_string());
        assert_eq!(err.to_string(), "Invalid URL: bad");

        let err = ClientError::Api("server error".to_string());
        assert_eq!(err.to_string(), "API error: server error");

        let err = ClientError::Timeout;
        assert_eq!(err.to_string(), "Request timeout");
    }

    #[test]
    fn test_client_error_from_serde_json() {
        let serde_err = serde_json::Error::io(std::io::Error::other("bad"));
        let err: ClientError = serde_err.into();
        assert!(matches!(err, ClientError::Serialization(_)));
    }

    #[test]
    fn test_client_error_from_url_parse() {
        let parse_err = url::ParseError::EmptyHost;
        let err: ClientError = parse_err.into();
        assert!(matches!(err, ClientError::UrlParse(_)));
        // TAG: surface=blockchain owner=blockchain-team rule=BC-001
    }

    #[tokio::test]
    async fn test_store_success() {
        let mock_server = MockServer::start().await;
        let mock_response = StoreResponse {
            hash: "abc123".to_string(),
            root: "root456".to_string(),
            metadata: None,
        };

        Mock::given(method("POST"))
            .and(path("/store"))
            .and(body_json(StoreRequest {
                data: "test data".to_string(),
                metadata: None,
            }))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        let client = NomtClient::new(mock_server.uri()).unwrap();
        let result = client.store("test data", None).await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.hash, "abc123");
        assert_eq!(resp.root, "root456");
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001

    #[tokio::test]
    async fn test_store_failure() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/store"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .mount(&mock_server)
            .await;

        let client = NomtClient::new(mock_server.uri()).unwrap();
        let result = client.store("test data", None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Store failed"));
    }

    #[tokio::test]
    async fn test_verify_success() {
        let mock_server = MockServer::start().await;
        let mock_response = VerifyResponse {
            verified: true,
            proof: Some(NomtProof {
                leaf_hash: "abc".to_string(),
                siblings: vec![],
                path: vec![],
                root: "root".to_string(),
            }),
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            current_root: "root".to_string(),
            error: None,
        };

        Mock::given(method("POST"))
            .and(path("/verify"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        let client = NomtClient::new(mock_server.uri()).unwrap();
        let result = client.verify("abc", None).await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert!(resp.verified);
    }

    #[tokio::test]
    async fn test_stats_success() {
        let mock_server = MockServer::start().await;
        let mock_response = StorageStats {
            total_items: 42,
            current_root: "root789".to_string(),
            last_updated: None,
            backend_type: "legacy".to_string(),
        };

        Mock::given(method("GET"))
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            .and(path("/stats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            .await;

        let client = NomtClient::new(mock_server.uri()).unwrap();
        let result = client.stats().await;
        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.total_items, 42);
        assert_eq!(stats.backend_type, "legacy");
    }

    #[tokio::test]
    async fn test_current_root_success() {
        let mock_server = MockServer::start().await;
        let mock_response = StorageStats {
            total_items: 1,
            current_root: "myroot".to_string(),
            last_updated: None,
            backend_type: "legacy".to_string(),
        };

        Mock::given(method("GET"))
            .and(path("/stats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&mock_response))
            .mount(&mock_server)
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            .await;

        let client = NomtClient::new(mock_server.uri()).unwrap();
        let result = client.current_root().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "myroot");
    }

    #[tokio::test]
    async fn test_prepare_anchor_success() {
        let mock_server = MockServer::start().await;
        let hash = "anchor_hash".to_string();
        let root = "anchor_root".to_string();

        let store_response = StoreResponse {
            hash: hash.clone(),
            root: root.clone(),
            metadata: None,
        };

        Mock::given(method("POST"))
            .and(path("/store"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&store_response))
            .mount(&mock_server)
            .await;

        let proof = NomtProof {
            leaf_hash: hash.clone(),
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            siblings: vec![],
            path: vec![],
            root: root.clone(),
        };
        let verify_response = VerifyResponse {
            verified: true,
            proof: Some(proof),
            current_root: root,
            error: None,
        };

        Mock::given(method("POST"))
            .and(path("/verify"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&verify_response))
            .mount(&mock_server)
            .await;

        let client = NomtClient::new(mock_server.uri()).unwrap();
        let result = client.prepare_anchor("data", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_prepare_anchor_no_proof() {
        let mock_server = MockServer::start().await;
        let store_response = StoreResponse {
            hash: "h".to_string(),
            // TAG: surface=blockchain owner=blockchain-team rule=BC-001
            root: "r".to_string(),
            metadata: None,
        };

        Mock::given(method("POST"))
            .and(path("/store"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&store_response))
            .mount(&mock_server)
            .await;

        let verify_response = VerifyResponse {
            verified: true,
            proof: None,
            current_root: "r".to_string(),
            error: None,
        };

        Mock::given(method("POST"))
            .and(path("/verify"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&verify_response))
            .mount(&mock_server)
            .await;

        let client = NomtClient::new(mock_server.uri()).unwrap();
        let result = client.prepare_anchor("data", None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No proof"));
    }

    #[test]
    fn test_nomt_client_debug_redacts_api_key() {
        let client = NomtClient::new("http://localhost:8081")
            .unwrap()
            .with_api_key("super-secret-sidecar-key");
        let dbg = format!("{client:?}");
        assert!(!dbg.contains("super-secret-sidecar-key"));
        assert!(dbg.contains("[REDACTED]"));
    }
    // TAG: surface=blockchain owner=blockchain-team rule=BC-001
}
