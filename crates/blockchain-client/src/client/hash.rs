//! Hash-related operations for the blockchain client

use anyhow::Result;
use sha2::Digest;
use tracing::{info, warn};

use crate::client::BlockchainClient;
use crate::helpers::generate_report_id;
use crate::report_type::ReportType;
use crate::types::{CreateHashRequest, CreateHashResponse, GetHashResponse, VerifyHashResponse};

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
impl BlockchainClient {
    /// Create a new hash on the blockchain
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn create_hash(&self, request: CreateHashRequest) -> Result<CreateHashResponse> {
        let sanitized_user = Self::sanitize_user_id_for_logs(&request.user_id);
        info!(
            user_id = %sanitized_user,
            report_type = %request.report_type,
            "Creating blockchain hash for user"
        );

        // Compute SHA-256 hash of the report data
        let data_json = serde_json::to_string(&request.report_data)?;
        let data_hash_hex = hex::encode(sha2::Sha256::digest(data_json.as_bytes()));
        let hash_bytes: [u8; 32] = sha2::Sha256::digest(data_json.as_bytes()).into();

        // Generate report_id
        let report_id = generate_report_id(&request.user_id, &request.report_type, &data_hash_hex);

        // Convert report type
        let report_type = ReportType::parse(&request.report_type);

        // Build the extrinsic
        let tx = crate::freshcredit_runtime::tx()
            .fresh_credit()
            .anchor_report_hash(report_id, hash_bytes, report_type.as_runtime_type());

        // Submit and wait for finalization
        let tx_progress = self
            .api
            .tx()
            .await?
            .sign_and_submit_then_watch_default(&tx, &*self.signer)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to submit transaction: {e}"))?;

        // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
        let events = tokio::time::timeout(
            crate::helpers::finalize_timeout(),
            tx_progress.wait_for_finalized_success(),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "Timed out after {}s waiting for transaction finalization",
                crate::helpers::finalize_timeout().as_secs()
            )
        })?
        .map_err(|e| anyhow::anyhow!("Transaction failed: {e}"))?;

        // Get block number from the latest header (transaction was just finalized)
        let _tx_hash = events.extrinsic_hash();
        let header = self.rpc.chain_get_header(None).await?;
        let block_number = header.map_or(0, |h| h.number);

        info!("Successfully anchored hash at block {}", block_number);

        Ok(CreateHashResponse {
            id: hex::encode(report_id),
            user_id: request.user_id,
            report_type: request.report_type,
            hash: data_hash_hex,
            block_number,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Get a hash by report ID (hex-encoded)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn get_hash(&self, _hash_id: &str) -> Result<Option<GetHashResponse>> {
        // Parse the hash_id to extract user_id and report_id
        // For now, we need both user_id and report_id to query storage
        // This is a limitation - we may need to add an index or change the API
        warn!("get_hash by ID alone requires storage iteration - not yet implemented");
        Ok(None)
    }

    /// Get all hashes for a user
    ///
    /// Note: Currently all hashes are stored under the service account (signer),
    /// not individual user accounts. This method iterates all hashes under the
    /// signer and filters by `user_id` in the `report_id` derivation.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_user_hashes(&self, user_id: &str) -> Result<Vec<GetHashResponse>> {
        // All hashes are stored under the service account (signer), not user_id
        let signer_account_id = self.get_signer_account_id();

        // Query storage for all report hashes under signer's account
        let at = self.api.at_current_block().await?;

        let mut hashes = Vec::new();
        // TAG: surface=blockchain owner=platform-team rule=GENERAL-001

        // Get the storage address for iteration under signer's account
        let address = crate::freshcredit_runtime::storage()
            .fresh_credit()
            .report_hashes();

        let mut iter = at.storage().iter(&address, (signer_account_id,)).await?;

        while let Some(Ok(kv)) = iter.next().await {
            // Extract report_id from the key
            let report_id_bytes = kv.key_bytes();
            let hash = kv.value().decode()?;

            // Get metadata for this report
            // Note: We'd need to parse the key to get report_id
            hashes.push(GetHashResponse {
                id: hex::encode(report_id_bytes),
                user_id: user_id.to_string(),
                report_type: "unknown".to_string(), // Would need metadata query
                hash: hex::encode(hash),
                block_number: 0, // Would need metadata query
                timestamp: String::new(),
                report_data: serde_json::Value::Null,
            });
        }

        Ok(hashes)
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Create a hash for a report (convenience method)
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn create_report_hash(
        &self,
        user_id: &str,
        report_type: &str,
        report_data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<CreateHashResponse> {
        let request = CreateHashRequest {
            user_id: user_id.to_string(),
            report_type: report_type.to_string(),
            report_data: serde_json::to_value(report_data)?,
        };

        self.create_hash(request).await
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Verify a hash exists on the blockchain
    ///
    /// This iterates through all hashes under the service account to find a match.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn verify_hash(&self, hash: &str) -> Result<VerifyHashResponse> {
        info!("Verifying hash on blockchain: {hash}");

        // Parse hash from hex to validate format
        let Some(hash_bytes) = crate::helpers::parse_h256_hex(hash) else {
            return Ok(VerifyHashResponse {
                verified: false,
                hash: hash.to_string(),
                block_number: None,
                timestamp: None,
                user_id: None,
                report_type: None,
            });
        };

        // All hashes are stored under the service account (signer)
        let signer_account_id = self.get_signer_account_id();
        let at = self.api.at_current_block().await?;

        // Iterate through all hashes to find a match
        let address = crate::freshcredit_runtime::storage()
            .fresh_credit()
            .report_hashes();

        let mut iter = at.storage().iter(&address, (signer_account_id,)).await?;

        while let Some(Ok(kv)) = iter.next().await {
            let stored_hash = kv.value().decode()?;
            if stored_hash == hash_bytes {
                // Found matching hash - get metadata
                // Extract report_id from key bytes (last 32 bytes after prefix)
                let report_id_bytes = crate::helpers::extract_report_id_from_key(kv.key_bytes());

                // Get metadata for this report
                let metadata = at
                    .storage()
                    .try_fetch(
                        &crate::freshcredit_runtime::storage()
                            .fresh_credit()
                            .report_metadata_store(),
                        (signer_account_id, report_id_bytes),
                    )
                    .await?;

                // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
                let (block_number, report_type) = match metadata {
                    Some(m) => {
                        let meta = m.decode()?;
                        (
                            Some(u64::from(meta.anchored_at)),
                            Some(format!("{:?}", meta.report_type)),
                        )
                    }
                    None => (None, None),
                };

                return Ok(VerifyHashResponse {
                    verified: true,
                    hash: hash.to_string(),
                    block_number,
                    timestamp: None,
                    user_id: None, // User ID not stored in pallet currently
                    report_type,
                });
            }
        }

        // Hash not found
        Ok(VerifyHashResponse {
            verified: false,
            hash: hash.to_string(),
            block_number: None,
            timestamp: None,
            user_id: None,
            report_type: None,
        })
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Verify a hash for a specific user and report
    ///
    /// Note: Hashes are stored under the service account, not user accounts.
    /// The `user_id` is used to derive the expected `report_id` for lookup.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn verify_user_hash(
        &self,
        user_id: &str,
        report_id: &str,
        expected_hash: &str,
    ) -> Result<VerifyHashResponse> {
        // Hashes are stored under service account, not user-derived account
        let signer_account_id = self.get_signer_account_id();

        // Parse report_id from hex
        let Some(report_id_bytes) = crate::helpers::parse_h256_hex(report_id) else {
            return Err(anyhow::anyhow!("Invalid report_id format"));
        };

        // Query storage under signer's account
        let at = self.api.at_current_block().await?;
        let stored_hash = at
            .storage()
            .try_fetch(
                &crate::freshcredit_runtime::storage()
                    .fresh_credit()
                    .report_hashes(),
                (signer_account_id, report_id_bytes),
            )
            .await?;

        match stored_hash {
            Some(hash) => {
                let stored_hex = hex::encode(hash.decode()?);
                let verified = stored_hex == expected_hash;

                // Get metadata
                let metadata = at
                    .storage()
                    .try_fetch(
                        &crate::freshcredit_runtime::storage()
                            .fresh_credit()
                            .report_metadata_store(),
                        (signer_account_id, report_id_bytes),
                    )
                    .await?;

                // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
                let (block_number, report_type) = match metadata {
                    Some(m) => {
                        let meta = m.decode()?;
                        (
                            Some(u64::from(meta.anchored_at)),
                            Some(format!("{:?}", meta.report_type)),
                        )
                    }
                    None => (None, None),
                };

                Ok(VerifyHashResponse {
                    verified,
                    hash: stored_hex,
                    block_number,
                    timestamp: None,
                    user_id: Some(user_id.to_string()),
                    report_type,
                })
            }
            None => Ok(VerifyHashResponse {
                verified: false,
                hash: expected_hash.to_string(),
                block_number: None,
                timestamp: None,
                user_id: Some(user_id.to_string()),
                report_type: None,
            }),
        }
    }
}
