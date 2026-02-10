//! Record types for Ethereum state data
//!
//! These structs represent the data stored in the state store.
//! They use postcard for binary serialization, which is compact and deterministic.

use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};

/// Account record containing nonce, balance, and code hash.
///
/// This matches the core account state in Ethereum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRecord {
    /// Transaction nonce (number of transactions sent from this account)
    pub nonce: u64,
    /// Account balance in wei
    pub balance: U256,
    /// Hash of the contract bytecode (B256::ZERO for EOA accounts)
    pub code_hash: B256,
}

/// Header record containing block metadata.
///
/// This provides the minimal block header information needed for EVM execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderRecord {
    /// Block number
    pub number: u64,
    /// Block timestamp (Unix epoch seconds)
    pub timestamp: u64,
    /// Base fee per gas (EIP-1559)
    pub basefee: U256,
    /// Coinbase address (block beneficiary)
    pub coinbase: Address,
    /// Previous RANDAO value (EIP-4399, replaces mixHash)
    pub prevrandao: B256,
    /// Gas limit for the block
    pub gas_limit: u64,
    /// Chain ID
    pub chain_id: u64,
}

/// Block delta record for an address in a specific block.
///
/// Only stored when the address's balance or nonce changed in that block.
/// Tracks all changes: received value, sent value, fees, and nonce increments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockDelta {
    /// Block number (helpful for verification, but also encoded in key)
    pub block: u64,
    /// Total value received in this block
    pub delta_plus: U256,
    /// Total value sent (including fees) in this block
    pub delta_minus: U256,
    /// Total value received from transfers (for analytics)
    pub received_value: U256,
    /// Total value sent in transfers (for analytics)
    pub sent_value: U256,
    /// Total fees paid for successful transactions
    pub fee_paid: U256,
    /// Total fees paid for failed transactions
    pub failed_fee: U256,
    /// Nonce increment (usually 0 or 1, but could be more if multiple txs in block)
    pub nonce_delta: u64,
    /// Number of transactions affecting this address in this block
    pub tx_count: u32,
}

impl BlockDelta {
    /// Create a new empty delta for a block.
    pub fn new(block: u64) -> Self {
        Self {
            block,
            delta_plus: U256::ZERO,
            delta_minus: U256::ZERO,
            received_value: U256::ZERO,
            sent_value: U256::ZERO,
            fee_paid: U256::ZERO,
            failed_fee: U256::ZERO,
            nonce_delta: 0,
            tx_count: 0,
        }
    }

    /// Check if this delta represents any changes (non-zero).
    pub fn has_changes(&self) -> bool {
        self.delta_plus > U256::ZERO
            || self.delta_minus > U256::ZERO
            || self.nonce_delta > 0
    }
}

/// Balance snapshot for an address at a specific block.
///
/// Stores the balance AFTER applying all transactions in that block.
/// Only stored when the address's balance changed in that block.
pub type BalanceSnapshot = U256;

/// Watch metadata for an address.
///
/// Tracks when we started watching this address.
/// Used to enforce coverage boundaries in queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchMeta {
    /// Block number at which we started tracking this address.
    pub start_block: u64,
}

/// Per-block ERC20 delta for a specific (token, owner) at a given block.
///
/// Keyed in RocksDB as:
///   'T' + token(20 bytes) + owner(20 bytes) + block(u64 BE)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Erc20Delta {
    /// Block number (also encoded in the key).
    pub block: u64,
    /// Tokens received in this block.
    pub delta_plus: U256,
    /// Tokens sent/burned in this block.
    pub delta_minus: U256,
    /// Number of Transfer events that affected this (token, owner) in this block.
    pub tx_count: u32,
}

impl Erc20Delta {
    /// Create a new empty delta for a block.
    pub fn new(block: u64) -> Self {
        Self {
            block,
            delta_plus: U256::ZERO,
            delta_minus: U256::ZERO,
            tx_count: 0,
        }
    }

    /// Whether this delta has any actual changes.
    pub fn has_changes(&self) -> bool {
        self.delta_plus > U256::ZERO || self.delta_minus > U256::ZERO
    }
}

/// ERC20 balance snapshot for (token, owner) at a specific block.
///
/// Keyed as:
///   'U' + token(20) + owner(20) + block(u64 BE)
pub type Erc20Snapshot = U256;

/// Coverage metadata for a (token, owner) pair.
///
/// This is similar to `WatchMeta` for ETH addresses, but scoped to
/// a token+owner combination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenWatchMeta {
    /// Block number at which we started tracking this (token, owner).
    pub start_block: u64,
}

/// Encode a U256 value as a fixed 32-byte big-endian byte array.
///
/// This ensures deterministic encoding for storage values and other U256 fields.
pub fn encode_u256(value: U256) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    value.to_be_bytes_vec().into_iter().enumerate().for_each(|(i, b)| {
        if i < 32 {
            bytes[i] = b;
        }
    });
    bytes
}

/// Decode a 32-byte big-endian byte array into a U256 value.
pub fn decode_u256(bytes: &[u8]) -> Result<U256, anyhow::Error> {
    if bytes.len() != 32 {
        anyhow::bail!("U256 encoding must be exactly 32 bytes, got {}", bytes.len());
    }
    Ok(U256::from_be_slice(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u256_encoding_roundtrip() {
        let value = U256::from(123456789u64);
        let encoded = encode_u256(value);
        let decoded = decode_u256(&encoded).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn test_u256_encoding_zero() {
        let value = U256::ZERO;
        let encoded = encode_u256(value);
        assert_eq!(encoded, [0u8; 32]);
        let decoded = decode_u256(&encoded).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn test_u256_encoding_large() {
        let value = U256::MAX;
        let encoded = encode_u256(value);
        let decoded = decode_u256(&encoded).unwrap();
        assert_eq!(value, decoded);
    }
}
