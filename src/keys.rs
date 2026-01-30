//! Key encoding and decoding utilities
//!
//! All keys use a single-byte prefix followed by binary data.
//! This ensures deterministic, lexicographically ordered keys in RocksDB.

use alloy_primitives::{Address, B256};

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
}
