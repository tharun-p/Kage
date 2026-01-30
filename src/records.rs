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
