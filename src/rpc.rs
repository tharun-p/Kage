//! JSON-RPC client for Ethereum nodes
//!
//! Provides a typed interface to Ethereum JSON-RPC endpoints.
//! Handles hex string parsing and error handling.

use crate::types::{Block, Receipt};
use alloy_primitives::{Address, B256, U256};
use anyhow::{Context, Result};
use serde_json::{json, Value};

/// JSON-RPC client for Ethereum nodes.
pub struct RpcClient {
    client: reqwest::Client,
    url: String,
}

impl RpcClient {
    /// Create a new RPC client.
    pub fn new(url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
        }
    }

    /// Make a JSON-RPC call.
    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .context("Failed to send RPC request")?;

        let json: Value = response
            .json()
            .await
            .context("Failed to parse RPC response")?;

        // Check for RPC error
        if let Some(error) = json.get("error") {
            anyhow::bail!("RPC error: {}", error);
        }

        // Extract result
        json.get("result")
            .cloned()
            .context("RPC response missing 'result' field")
    }

    /// Get a block by number with full transaction details.
    ///
    /// `block` can be a block number (u64) or "finalized", "latest", etc.
    /// `full_tx` should be true to get full transaction objects.
    pub async fn get_block_by_number(&self, block: &str, full_tx: bool) -> Result<Block> {
        let params = json!([block, full_tx]);
        let result = self.call("eth_getBlockByNumber", params).await?;
        serde_json::from_value(result).context("Failed to deserialize block")
    }

    /// Get just the block number for a given block tag.
    ///
    /// `block` can be "finalized", "latest", etc.
    /// This is more efficient than fetching the full block when you only need the number.
    pub async fn get_block_number(&self, block: &str) -> Result<u64> {
        let params = json!([block, false]);
        let result = self.call("eth_getBlockByNumber", params).await?;
        
        // Extract number field from block
        let number_str = result
            .get("number")
            .and_then(|v| v.as_str())
            .context("Block missing 'number' field")?;
        
        let number_str = number_str.strip_prefix("0x").unwrap_or(number_str);
        if number_str.is_empty() {
            anyhow::bail!("Block number is empty");
        }
        u64::from_str_radix(number_str, 16)
            .context("Failed to parse block number")
    }

    /// Get the current finalized block number.
    ///
    /// Tries "finalized" first, then falls back to "latest" if finalized is not available
    /// (e.g., on local test nodes like Anvil).
    pub async fn get_finalized_block_number(&self) -> Result<u64> {
        // Try "finalized" first
        let params = json!(["finalized", false]);
        let result = self.call("eth_getBlockByNumber", params).await;
        
        // Check if we got a valid block (not null)
        let block = match result {
            Ok(ref json) if json.is_null() => {
                // "finalized" returned null, try "latest"
                tracing::debug!("'finalized' block tag not available, falling back to 'latest'");
                None
            }
            Ok(json) => Some(json),
            Err(_) => {
                // RPC error, try "latest" as fallback
                tracing::debug!("'finalized' block tag failed, falling back to 'latest'");
                None
            }
        };
        
        let result = if let Some(block) = block {
            block
        } else {
            // Fallback to "latest"
            let params = json!(["latest", false]);
            self.call("eth_getBlockByNumber", params).await?
        };
        
        // Extract number field from block
        let number_str = result
            .get("number")
            .and_then(|v| v.as_str())
            .context("Block missing 'number' field")?;
        
        let number_str = number_str.strip_prefix("0x").unwrap_or(number_str);
        if number_str.is_empty() {
            anyhow::bail!("Block number is empty");
        }
        u64::from_str_radix(number_str, 16)
            .context("Failed to parse block number")
    }

    /// Get a transaction receipt by hash.
    pub async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Receipt> {
        let hash_str = format!("0x{:x}", tx_hash);
        let params = json!([hash_str]);
        let result = self.call("eth_getTransactionReceipt", params).await?;
        serde_json::from_value(result).context("Failed to deserialize receipt")
    }

    /// Get the balance of an address at a specific block.
    ///
    /// `block` can be a block number (u64) or "finalized", "latest", etc.
    pub async fn get_balance(&self, address: Address, block: &str) -> Result<U256> {
        let addr_str = format!("0x{:x}", address);
        let params = json!([addr_str, block]);
        let result = self.call("eth_getBalance", params).await?;
        
        let balance_str_raw = result
            .as_str()
            .context("Balance response is not a string")?;
        
        tracing::debug!("RPC get_balance({:?}, {}) returned raw: {}", address, block, balance_str_raw);
        
        let balance_str = balance_str_raw.strip_prefix("0x").unwrap_or(balance_str_raw);
        if balance_str.is_empty() {
            return Ok(U256::ZERO);
        }
        
        // Handle odd-length hex strings by padding with a leading zero
        let balance_str = if balance_str.len() % 2 == 1 {
            format!("0{}", balance_str)
        } else {
            balance_str.to_string()
        };
        
        let bytes = hex::decode(&balance_str).context("Failed to decode balance hex")?;
        let balance = U256::from_be_slice(&bytes);
        tracing::debug!("Parsed balance: {} (from hex: {})", balance, balance_str);
        Ok(balance)
    }

    /// Get the transaction count (nonce) of an address at a specific block.
    ///
    /// `block` can be a block number (u64) or "finalized", "latest", etc.
    pub async fn get_transaction_count(&self, address: Address, block: &str) -> Result<u64> {
        let addr_str = format!("0x{:x}", address);
        let params = json!([addr_str, block]);
        let result = self.call("eth_getTransactionCount", params).await?;
        
        let count_str = result
            .as_str()
            .context("Transaction count response is not a string")?;
        
        let count_str = count_str.strip_prefix("0x").unwrap_or(count_str);
        if count_str.is_empty() {
            return Ok(0);
        }
        u64::from_str_radix(count_str, 16)
            .context("Failed to parse transaction count")
    }

    /// Get the code at an address at a specific block.
    ///
    /// Returns empty Vec for EOA addresses, contract bytecode for contracts.
    /// `block` can be a block number (u64) or "finalized", "latest", etc.
    pub async fn get_code(&self, address: Address, block: &str) -> Result<Vec<u8>> {
        let addr_str = format!("0x{:x}", address);
        let params = json!([addr_str, block]);
        let result = self.call("eth_getCode", params).await?;
        
        let code_str = result
            .as_str()
            .context("Code response is not a string")?;
        
        let code_str = code_str.strip_prefix("0x").unwrap_or(code_str);
        if code_str.is_empty() {
            return Ok(Vec::new());
        }
        
        // Handle odd-length hex strings by padding with a leading zero
        let code_str = if code_str.len() % 2 == 1 {
            format!("0{}", code_str)
        } else {
            code_str.to_string()
        };
        
        hex::decode(&code_str).context("Failed to decode code hex")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_formatting() {
        // Test that address formatting works correctly
        let addr_bytes = hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap();
        let addr = Address::from_slice(&addr_bytes);
        assert_eq!(format!("0x{:x}", addr), "0x0742d35cc6634c0532925a3b844bc9e7595f0beb");
    }
}
