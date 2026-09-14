//! NOMT (Nearly-Optimal Merkle Trie) sidecar client
//!
//! Client for interacting with the NOMT sidecar for off-chain state storage.
//! NOMT provides high-performance merklized key-value storage with Merkle proofs.
//!
//! ## Architecture
//! - NOMT sidecar runs alongside Substrate node
//! - Stores off-chain state with Merkle proofs
//! - Anchors root hash on-chain for verification

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Response from NOMT verify endpoint
#[derive(Deserialize)]
struct VerifyResponse {
    verified: bool,
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// NOMT Merkle proof for off-chain state verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NomtProof {
    /// Hash of the leaf node (the stored data)
    pub leaf_hash: String,
    /// Sibling hashes along the path to root
    pub siblings: Vec<String>,
    /// Path direction at each level (true = right, false = left)
    pub path: Vec<bool>,
    /// Root hash this proof verifies against
    pub root: String,
}

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
/// Result from storing data with NOMT proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NomtAnchorResult {
    /// SHA-256 hash of the stored data
    pub hash: String,
    /// NOMT root hash after insertion
    pub nomt_root: String,
    /// Block number where anchor was stored on-chain
    pub block_number: u64,
    /// Merkle proof for the stored data
    pub proof: NomtProof,
}

/// NOMT sidecar client for off-chain state storage
#[derive(Clone, Debug)]
pub struct NomtClient {
    /// HTTP client for NOMT sidecar API
    http_client: reqwest::Client,
    /// NOMT sidecar URL
    sidecar_url: String,
}

impl NomtClient {
    /// Create a new NOMT client
    #[must_use]
    pub fn new(sidecar_url: &str) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            sidecar_url: sidecar_url.to_string(),
        }
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Store report data in NOMT and return proof
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn store(
        &self,
        user_id: &str,
        report_type: &str,
        data: &[u8],
    ) -> Result<NomtAnchorResult> {
        let url = format!("{}/store", self.sidecar_url);

        let response = self
            .http_client
            .post(&url)
            .json(&serde_json::json!({
                "user_id": user_id,
                "report_type": report_type,
                "data": data
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("NOMT store failed: {}", response.status());
        }

        let result: NomtAnchorResult = response.json().await?;
        Ok(result)
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Get Merkle proof for a hash
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_proof(&self, hash: &str) -> Result<Option<NomtProof>> {
        let url = format!("{}/proof/{}", self.sidecar_url, hash);

        let response = self.http_client.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            anyhow::bail!("NOMT get_proof failed: {}", response.status());
        }

        let proof: NomtProof = response.json().await?;
        Ok(Some(proof))
    }

    /// Verify a hash exists in NOMT storage
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn verify(&self, hash: &str) -> Result<bool> {
        let url = format!("{}/verify/{}", self.sidecar_url, hash);

        let response = self.http_client.get(&url).send().await?;

        if !response.status().is_success() {
            return Ok(false);
        }

        let result: VerifyResponse = response.json().await?;
        Ok(result.verified)
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Check if NOMT sidecar is healthy
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.sidecar_url);

        (self.http_client.get(&url).send().await)
            .map_or_else(|_| Ok(false), |response| Ok(response.status().is_success()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[test]
    fn test_nomt_client_new() {
        let client = NomtClient::new("http://localhost:8080");
        assert!(std::mem::size_of_val(&client) > 0);
    }

    #[tokio::test]
    async fn test_store_success() {
        let mock_server = MockServer::start().await;
        let result = NomtAnchorResult {
            hash: "abc123".to_string(),
            // TAG: surface=blockchain owner=platform-team rule=GENERAL-001
            nomt_root: "root456".to_string(),
            block_number: 42,
            proof: NomtProof {
                leaf_hash: "abc123".to_string(),
                siblings: vec!["sib1".to_string()],
                path: vec![false, true],
                root: "root456".to_string(),
            },
        };

        Mock::given(method("POST"))
            .and(path("/store"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&result))
            .mount(&mock_server)
            .await;

        let client = NomtClient::new(&mock_server.uri());
        let resp = client.store("user-1", "report", b"data").await.unwrap();
        assert_eq!(resp.hash, "abc123");
        assert_eq!(resp.block_number, 42);
    }

    #[tokio::test]
    async fn test_store_failure() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/store"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
        let client = NomtClient::new(&mock_server.uri());
        let result = client.store("user-1", "report", b"data").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_proof_success() {
        let mock_server = MockServer::start().await;
        let proof = NomtProof {
            leaf_hash: "abc".to_string(),
            siblings: vec!["s1".to_string()],
            path: vec![true],
            root: "root".to_string(),
        };

        Mock::given(method("GET"))
            .and(path("/proof/abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&proof))
            .mount(&mock_server)
            .await;

        let client = NomtClient::new(&mock_server.uri());
        let result = client.get_proof("abc").await.unwrap();
        assert!(result.is_some());
        let p = result.unwrap();
        assert_eq!(p.leaf_hash, "abc");
    }

    #[tokio::test]
    async fn test_get_proof_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/proof/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
        let client = NomtClient::new(&mock_server.uri());
        let result = client.get_proof("missing").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_verify_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/verify/abc"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"verified": true})),
            )
            .mount(&mock_server)
            .await;

        let client = NomtClient::new(&mock_server.uri());
        let result = client.verify("abc").await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_verify_failure() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/verify/bad"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = NomtClient::new(&mock_server.uri());
        let result = client.verify("bad").await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_health_check_success() {
        let mock_server = MockServer::start().await;

        // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = NomtClient::new(&mock_server.uri());
        assert!(client.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_health_check_failure() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let client = NomtClient::new(&mock_server.uri());
        assert!(!client.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_health_check_connection_error() {
        // Use a port that's unlikely to be open
        let client = NomtClient::new("http://localhost:1");
        assert!(!client.health_check().await.unwrap());
    }
}
