//! Key encoding and decoding utilities
//!
//! All keys use a single-byte prefix followed by binary data.
//! This ensures deterministic, lexicographically ordered keys in RocksDB.

use alloy_primitives::{Address, B256};
use anyhow;

/// Encode an account key.
///
/// Format: byte 'A' (0x41) + address (20 bytes)
/// Total length: 21 bytes
pub fn encode_account_key(addr: Address) -> Vec<u8> {
    let mut key = Vec::with_capacity(21);
    key.push(b'A');
    key.extend_from_slice(addr.as_slice());
    key
}

/// Encode a code key.
///
/// Format: byte 'C' (0x43) + code_hash (32 bytes)
/// Total length: 33 bytes
pub fn encode_code_key(code_hash: B256) -> Vec<u8> {
    let mut key = Vec::with_capacity(33);
    key.push(b'C');
    key.extend_from_slice(code_hash.as_slice());
    key
}

/// Encode a storage key.
///
/// Format: byte 'S' (0x53) + address (20 bytes) + slot (32 bytes)
/// Total length: 53 bytes
pub fn encode_storage_key(addr: Address, slot: B256) -> Vec<u8> {
    let mut key = Vec::with_capacity(53);
    key.push(b'S');
    key.extend_from_slice(addr.as_slice());
    key.extend_from_slice(slot.as_slice());
    key
}

/// Encode a header key.
///
/// Format: byte 'H' (0x48) + block_number (8 bytes, big-endian)
/// Total length: 9 bytes
pub fn encode_header_key(block: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(9);
    key.push(b'H');
    key.extend_from_slice(&block.to_be_bytes());
    key
}

/// Encode a block hash key.
///
/// Format: byte 'B' (0x42) + block_number (8 bytes, big-endian)
/// Total length: 9 bytes
pub fn encode_block_hash_key(block: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(9);
    key.push(b'B');
    key.extend_from_slice(&block.to_be_bytes());
    key
}

/// Encode a meta key.
///
/// Format: byte 'M' (0x4D) + meta_id (1 byte)
/// Total length: 2 bytes
///
/// Meta IDs:
/// - 0x01: head_block
pub fn encode_meta_key(meta_id: u8) -> Vec<u8> {
    vec![b'M', meta_id]
}

/// Encode a block delta key (address-first for efficient prefix scans).
///
/// Format: byte 'D' (0x44) + address (20 bytes) + block_number (8 bytes, big-endian)
/// Total length: 29 bytes
///
/// This ordering allows efficient queries: all deltas for an address are contiguous.
pub fn encode_delta_key(addr: Address, block: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(29);
    key.push(b'D');
    key.extend_from_slice(addr.as_slice());
    key.extend_from_slice(&block.to_be_bytes());
    key
}

/// Decode a block delta key.
///
/// Returns (address, block_number) if the key is valid.
pub fn decode_delta_key(key: &[u8]) -> Result<(Address, u64), anyhow::Error> {
    if key.len() != 29 {
        anyhow::bail!("Delta key must be 29 bytes, got {}", key.len());
    }
    if key[0] != b'D' {
        anyhow::bail!("Invalid delta key prefix");
    }
    let addr = Address::from_slice(&key[1..21]);
    let block = u64::from_be_bytes(
        key[21..29]
            .try_into()
            .map_err(|_| anyhow::anyhow!("Failed to parse block number"))?,
    );
    Ok((addr, block))
}

/// Encode a balance snapshot key (address-first for efficient prefix scans).
///
/// Format: byte 'S' (0x53) + address (20 bytes) + block_number (8 bytes, big-endian)
/// Total length: 29 bytes
///
/// Note: 'S' is also used for storage keys, but storage keys are 53 bytes
/// (address + slot), so there's no conflict.
///
/// This ordering allows efficient queries: all snapshots for an address are contiguous.
pub fn encode_snapshot_key(addr: Address, block: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(29);
    key.push(b'Z'); // Use 'Z' to avoid conflict with storage key 'S'
    key.extend_from_slice(addr.as_slice());
    key.extend_from_slice(&block.to_be_bytes());
    key
}

/// Decode a balance snapshot key.
///
/// Returns (address, block_number) if the key is valid.
pub fn decode_snapshot_key(key: &[u8]) -> Result<(Address, u64), anyhow::Error> {
    if key.len() != 29 {
        anyhow::bail!("Snapshot key must be 29 bytes, got {}", key.len());
    }
    if key[0] != b'Z' {
        anyhow::bail!("Invalid snapshot key prefix");
    }
    let addr = Address::from_slice(&key[1..21]);
    let block = u64::from_be_bytes(
        key[21..29]
            .try_into()
            .map_err(|_| anyhow::anyhow!("Failed to parse block number"))?,
    );
    Ok((addr, block))
}

/// Encode a watch metadata key.
///
/// Format: byte 'W' (0x57) + address (20 bytes)
/// Total length: 21 bytes
pub fn encode_watch_meta_key(addr: Address) -> Vec<u8> {
    let mut key = Vec::with_capacity(21);
    key.push(b'W');
    key.extend_from_slice(addr.as_slice());
    key
}

/// Decode a watch metadata key.
///
/// Returns the address if the key is valid.
pub fn decode_watch_meta_key(key: &[u8]) -> Result<Address, anyhow::Error> {
    if key.len() != 21 {
        anyhow::bail!("Watch meta key must be 21 bytes, got {}", key.len());
    }
    if key[0] != b'W' {
        anyhow::bail!("Invalid watch meta key prefix");
    }
    Ok(Address::from_slice(&key[1..21]))
}

// -----------------------------------------------------------------------------
// ERC20 keys
// -----------------------------------------------------------------------------

/// Encode an ERC20 delta key.
///
/// Format: 'T' (0x54) + token(20 bytes) + owner(20 bytes) + block(u64 BE)
/// Total length: 49 bytes
pub fn encode_erc20_delta_key(token: Address, owner: Address, block: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(1 + 20 + 20 + 8);
    key.push(b'T');
    key.extend_from_slice(token.as_slice());
    key.extend_from_slice(owner.as_slice());
    key.extend_from_slice(&block.to_be_bytes());
    key
}

/// Decode an ERC20 delta key back to (token, owner, block).
pub fn decode_erc20_delta_key(key: &[u8]) -> Result<(Address, Address, u64), anyhow::Error> {
    if key.len() != 1 + 20 + 20 + 8 {
        anyhow::bail!("ERC20 delta key must be 49 bytes, got {}", key.len());
    }
    if key[0] != b'T' {
        anyhow::bail!("Invalid ERC20 delta key prefix");
    }
    let token = Address::from_slice(&key[1..21]);
    let owner = Address::from_slice(&key[21..41]);
    let block = u64::from_be_bytes(
        key[41..49]
            .try_into()
            .map_err(|_| anyhow::anyhow!("Failed to parse block number"))?,
    );
    Ok((token, owner, block))
}

/// Encode an ERC20 snapshot key.
///
/// Format: 'U' (0x55) + token(20 bytes) + owner(20 bytes) + block(u64 BE)
/// Total length: 49 bytes
pub fn encode_erc20_snapshot_key(token: Address, owner: Address, block: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(1 + 20 + 20 + 8);
    key.push(b'U');
    key.extend_from_slice(token.as_slice());
    key.extend_from_slice(owner.as_slice());
    key.extend_from_slice(&block.to_be_bytes());
    key
}

/// Decode an ERC20 snapshot key back to (token, owner, block).
pub fn decode_erc20_snapshot_key(key: &[u8]) -> Result<(Address, Address, u64), anyhow::Error> {
    if key.len() != 1 + 20 + 20 + 8 {
        anyhow::bail!("ERC20 snapshot key must be 49 bytes, got {}", key.len());
    }
    if key[0] != b'U' {
        anyhow::bail!("Invalid ERC20 snapshot key prefix");
    }
    let token = Address::from_slice(&key[1..21]);
    let owner = Address::from_slice(&key[21..41]);
    let block = u64::from_be_bytes(
        key[41..49]
            .try_into()
            .map_err(|_| anyhow::anyhow!("Failed to parse block number"))?,
    );
    Ok((token, owner, block))
}

/// Encode a token watch meta key.
///
/// Format: 'X' (0x58) + token(20 bytes) + owner(20 bytes)
/// Total length: 41 bytes
pub fn encode_token_watch_meta_key(token: Address, owner: Address) -> Vec<u8> {
    let mut key = Vec::with_capacity(1 + 20 + 20);
    key.push(b'X');
    key.extend_from_slice(token.as_slice());
    key.extend_from_slice(owner.as_slice());
    key
}

/// Decode token watch meta key back to (token, owner).
pub fn decode_token_watch_meta_key(key: &[u8]) -> Result<(Address, Address), anyhow::Error> {
    if key.len() != 1 + 20 + 20 {
        anyhow::bail!("Token watch meta key must be 41 bytes, got {}", key.len());
    }
    if key[0] != b'X' {
        anyhow::bail!("Invalid token watch meta key prefix");
    }
    let token = Address::from_slice(&key[1..21]);
    let owner = Address::from_slice(&key[21..41]);
    Ok((token, owner))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, b256};
    use hex;

    #[test]
    fn test_account_key_encoding() {
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let key = encode_account_key(addr);
        assert_eq!(key.len(), 21);
        assert_eq!(key[0], b'A');
        assert_eq!(&key[1..], addr.as_slice());
    }

    #[test]
    fn test_code_key_encoding() {
        let hash = b256!("0000000000000000000000000000000000000000000000000000000000000001");
        let key = encode_code_key(hash);
        assert_eq!(key.len(), 33);
        assert_eq!(key[0], b'C');
        assert_eq!(&key[1..], hash.as_slice());
    }

    #[test]
    fn test_storage_key_encoding() {
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let slot = b256!("0000000000000000000000000000000000000000000000000000000000000000");
        let key = encode_storage_key(addr, slot);
        assert_eq!(key.len(), 53);
        assert_eq!(key[0], b'S');
        assert_eq!(&key[1..21], addr.as_slice());
        assert_eq!(&key[21..], slot.as_slice());
    }

    #[test]
    fn test_header_key_encoding() {
        let block = 12345u64;
        let key = encode_header_key(block);
        assert_eq!(key.len(), 9);
        assert_eq!(key[0], b'H');
        assert_eq!(u64::from_be_bytes(key[1..9].try_into().unwrap()), block);
    }

    #[test]
    fn test_block_hash_key_encoding() {
        let block = 67890u64;
        let key = encode_block_hash_key(block);
        assert_eq!(key.len(), 9);
        assert_eq!(key[0], b'B');
        assert_eq!(u64::from_be_bytes(key[1..9].try_into().unwrap()), block);
    }

    #[test]
    fn test_meta_key_encoding() {
        let key = encode_meta_key(0x01);
        assert_eq!(key.len(), 2);
        assert_eq!(key[0], b'M');
        assert_eq!(key[1], 0x01);
    }

    #[test]
    fn test_delta_key_encoding() {
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let block = 12345u64;
        let key = encode_delta_key(addr, block);
        assert_eq!(key.len(), 29);
        assert_eq!(key[0], b'D');
        assert_eq!(&key[1..21], addr.as_slice());
        assert_eq!(u64::from_be_bytes(key[21..29].try_into().unwrap()), block);
    }

    #[test]
    fn test_delta_key_roundtrip() {
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let block = 67890u64;
        let key = encode_delta_key(addr, block);
        let (decoded_addr, decoded_block) = decode_delta_key(&key).unwrap();
        assert_eq!(addr, decoded_addr);
        assert_eq!(block, decoded_block);
    }

    #[test]
    fn test_snapshot_key_encoding() {
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let block = 12345u64;
        let key = encode_snapshot_key(addr, block);
        assert_eq!(key.len(), 29);
        assert_eq!(key[0], b'Z');
        assert_eq!(&key[1..21], addr.as_slice());
        assert_eq!(u64::from_be_bytes(key[21..29].try_into().unwrap()), block);
    }

    #[test]
    fn test_snapshot_key_roundtrip() {
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let block = 67890u64;
        let key = encode_snapshot_key(addr, block);
        let (decoded_addr, decoded_block) = decode_snapshot_key(&key).unwrap();
        assert_eq!(addr, decoded_addr);
        assert_eq!(block, decoded_block);
    }

    #[test]
    fn test_address_first_key_ordering() {
        // Test that address-first keys allow efficient prefix scanning
        let addr1 = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let addr2 = Address::from_slice(&hex::decode("dAC17F958D2ee523a2206206994597C13D831ec7").unwrap());
        
        let key1_block10 = encode_delta_key(addr1, 10);
        let key1_block20 = encode_delta_key(addr1, 20);
        let key2_block10 = encode_delta_key(addr2, 10);
        
        // All keys for addr1 should be contiguous (addr1 < addr2 lexicographically)
        assert!(key1_block10 < key1_block20);
        assert!(key1_block20 < key2_block10);
    }

    #[test]
    fn test_erc20_delta_key_roundtrip() {
        let token = Address::from_slice(&hex::decode("dAC17F958D2ee523a2206206994597C13D831ec7").unwrap());
        let owner = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let block = 100u64;
        let key = encode_erc20_delta_key(token, owner, block);
        let (t, o, b) = decode_erc20_delta_key(&key).unwrap();
        assert_eq!(token, t);
        assert_eq!(owner, o);
        assert_eq!(block, b);
    }

    #[test]
    fn test_erc20_snapshot_key_roundtrip() {
        let token = Address::from_slice(&hex::decode("dAC17F958D2ee523a2206206994597C13D831ec7").unwrap());
        let owner = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let block = 100u64;
        let key = encode_erc20_snapshot_key(token, owner, block);
        let (t, o, b) = decode_erc20_snapshot_key(&key).unwrap();
        assert_eq!(token, t);
        assert_eq!(owner, o);
        assert_eq!(block, b);
    }

    #[test]
    fn test_token_watch_meta_key_roundtrip() {
        let token = Address::from_slice(&hex::decode("dAC17F958D2ee523a2206206994597C13D831ec7").unwrap());
        let owner = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let key = encode_token_watch_meta_key(token, owner);
        let (t, o) = decode_token_watch_meta_key(&key).unwrap();
        assert_eq!(token, t);
        assert_eq!(owner, o);
    }
}
