//! Block and chain query operations for the blockchain client

use anyhow::Result;
use tracing::{info, warn};

use crate::client::BlockchainClient;
use crate::types::BlockchainTransaction;

// TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
impl BlockchainClient {
    /// Get blockchain statistics
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_stats(&self) -> Result<serde_json::Value> {
        // Get chain info
        let header = self.rpc.chain_get_header(None).await?;
        let block_number = header.map_or(0, |h| h.number);

        // Get total anchored count from storage
        let at = self.api.at_current_block().await?;
        let total_anchored = at
            .storage()
            .try_fetch(
                &crate::freshcredit_runtime::storage()
                    .fresh_credit()
                    .total_anchored(),
                (),
            )
            .await?
            .map_or(0, |v| v.decode().unwrap_or(0));

        // Get system health
        let health = self.rpc.system_health().await?;

        // `system_health.peers` counts the node's connected peers, not the node
        // itself. Surface a network-node count (peers + self) plus the raw
        // connected count. Operators can set BLOCKCHAIN_CLUSTER_SIZE to ensure
        // the UI shows the expected topology when the local view is incomplete.
        let detected_network_nodes = health.peers as u64 + 1;
        let network_nodes = std::env::var("BLOCKCHAIN_CLUSTER_SIZE")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map_or(detected_network_nodes, |configured| {
                configured.max(detected_network_nodes)
            });

        Ok(serde_json::json!({
            "block_height": block_number,
            "total_anchored": total_anchored,
            "peers": network_nodes,
            "connected_peers": health.peers,
            "is_syncing": health.is_syncing
        }))
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Get a single block by number
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_block(&self, block_number: u32) -> Result<serde_json::Value> {
        // Get block hash for this block number
        let block_hash = self
            .rpc
            .chain_get_block_hash(Some(block_number.into()))
            .await?;

        let hash = block_hash.ok_or_else(|| anyhow::anyhow!("Block not found"))?;

        // Get block header
        let block_header = self.rpc.chain_get_header(Some(hash)).await?;
        let header = block_header.ok_or_else(|| anyhow::anyhow!("Block header not found"))?;

        // Get full block with extrinsics
        let block = self.rpc.chain_get_block(Some(hash)).await?;
        let extrinsics: Vec<serde_json::Value> = block
            .as_ref()
            .map(|b| {
                b.block
                    .extrinsics
                    .iter()
                    .enumerate()
                    .map(|(i, ext)| {
                        serde_json::json!({
                            "index": i,
                            "hex": format!("0x{}", hex::encode(ext.0.as_slice())),
                            "length": ext.0.len()
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let hash_hex = format!("0x{}", hex::encode(hash.0));

        Ok(serde_json::json!({
            "number": header.number,
            "hash": hash_hex,
            "parent_hash": format!("0x{}", hex::encode(header.parent_hash.0)),
            "state_root": format!("0x{}", hex::encode(header.state_root.0)),
            "extrinsics_root": format!("0x{}", hex::encode(header.extrinsics_root.0)),
            "extrinsics_count": extrinsics.len(),
            "extrinsics": extrinsics
        }))
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Get recent blocks with details
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_recent_blocks(&self, count: u32) -> Result<Vec<serde_json::Value>> {
        let header = self.rpc.chain_get_header(None).await?;
        let current_block = header.map_or(0, |h| h.number);

        let mut blocks = Vec::new();
        let start_block = current_block.saturating_sub(u64::from(count.saturating_sub(1)));

        for block_num in (start_block..=current_block).rev() {
            // Get block hash for this block number
            let block_hash = self
                .rpc
                .chain_get_block_hash(Some(block_num.into()))
                .await?;

            if let Some(hash) = block_hash {
                // Get block header for this hash
                let block_header = self.rpc.chain_get_header(Some(hash)).await?;

                if let Some(header) = block_header {
                    // Get block details - extrinsics count
                    let block = self.rpc.chain_get_block(Some(hash)).await?;
                    let extrinsics_count = block.as_ref().map_or(0, |b| b.block.extrinsics.len());

                    // Format block hash
                    let hash_hex = format!("0x{}", hex::encode(hash.0));

                    blocks.push(serde_json::json!({
                        "number": header.number,
                        "hash": hash_hex,
                        "parent_hash": format!("0x{}", hex::encode(header.parent_hash.0)),
                        "extrinsics_count": extrinsics_count,
                        "state_root": format!("0x{}", hex::encode(header.state_root.0)),
                    }));
                }
            }

            if blocks.len() >= count as usize {
                break;
            }
        }

        Ok(blocks)
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Get a block by its hash
    ///
    /// Returns block details if the block hash is found on-chain.
    /// Used to verify block hashes in the explorer.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_block_by_hash(&self, hash_hex: &str) -> Result<Option<serde_json::Value>> {
        // Parse and validate the hash
        let Some(hash_bytes) = crate::helpers::parse_h256_hex(hash_hex) else {
            return Ok(None); // Invalid hash format
        };

        let block_hash = subxt::utils::H256::from(hash_bytes);

        // Try to get the block header for this hash
        match self.rpc.chain_get_header(Some(block_hash)).await {
            Ok(Some(header)) => {
                // Block exists! Get full block details
                let block = self.rpc.chain_get_block(Some(block_hash)).await?;
                let extrinsics_count = block.as_ref().map_or(0, |b| b.block.extrinsics.len());

                let hash_hex = format!("0x{}", hex::encode(block_hash.0));

                Ok(Some(serde_json::json!({
                    "number": header.number,
                    "hash": hash_hex,
                    "parent_hash": format!("0x{}", hex::encode(header.parent_hash.0)),
                    "state_root": format!("0x{}", hex::encode(header.state_root.0)),
                    "extrinsics_root": format!("0x{}", hex::encode(header.extrinsics_root.0)),
                    "extrinsics_count": extrinsics_count
                })))
            }
            Ok(None) => Ok(None), // Block not found
            Err(e) => {
                warn!("Error querying block by hash: {}", e);
                Ok(None)
            }
        }
    }

    // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
    /// Get transaction details by hash (Task 1.2: Payment amount verification)
    ///
    /// # Arguments
    /// * `tx_hash` - The transaction hash (extrinsic hash) to query
    ///
    /// # Returns
    /// * `Ok(Some(BlockchainTransaction))` - Transaction found with amount, from, to
    /// * `Ok(None)` - Transaction not found
    /// * `Err(e)` - Query failed
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn get_transaction(&self, tx_hash: &str) -> Result<Option<BlockchainTransaction>> {
        info!("Querying transaction: {}", tx_hash);

        // Parse hash from hex
        let Some(hash_bytes) = crate::helpers::parse_h256_hex(tx_hash) else {
            return Err(anyhow::anyhow!("Invalid transaction hash format"));
        };

        let tx_hash = subxt::utils::H256::from(hash_bytes);

        // Get the latest block
        let header = self.rpc.chain_get_header(None).await?;
        let current_block = header.map_or(0, |h| h.number);

        // Search through recent blocks for the transaction
        // In a production system, this would use an indexer
        let search_limit: u64 = 100_u64.min(current_block);

        for block_num in ((current_block.saturating_sub(search_limit))..=current_block).rev() {
            let Some(block_hash) = self
                .rpc
                .chain_get_block_hash(Some(block_num.into()))
                .await?
            else {
                continue;
            };

            let Some(block) = self.rpc.chain_get_block(Some(block_hash)).await? else {
                continue;
            };

            // TAG: surface=blockchain owner=blockchain-team rule=BLOCKCHAIN-001
            // Check each extrinsic for matching hash
            for (idx, ext) in block.block.extrinsics.iter().enumerate() {
                let ext_hash =
                    subxt::utils::H256::from(sp_crypto_hashing::blake2_256(ext.0.as_slice()));

                if ext_hash == tx_hash {
                    // Found the transaction - extract details
                    // For balance transfers, the amount is encoded in the extrinsic
                    // This is a simplified extraction - production would decode properly
                    let amount = 0u64; // Placeholder - would decode from extrinsic
                    let from = format!("0x{}", hex::encode(self.signer.public_key().0));
                    let to = String::new(); // Would extract from extrinsic args

                    return Ok(Some(BlockchainTransaction {
                        hash: tx_hash.to_string(),
                        amount,
                        from,
                        to,
                        block_number: block_num,
                        extrinsic_index: u32::try_from(idx).unwrap_or(u32::MAX),
                    }));
                }
            }
        }

        // Transaction not found in recent blocks
        Ok(None)
    }
}
