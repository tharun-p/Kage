//! StateStore trait and RocksDB implementation
//!
//! Provides a persistent key-value store for Ethereum state data.
//! Uses RocksDB with column families for efficient organization.

use crate::keys::{
    encode_account_key, encode_block_hash_key, encode_code_key, encode_header_key,
    encode_meta_key, encode_storage_key,
};
use crate::records::{decode_u256, encode_u256, AccountRecord, HeaderRecord};
use alloy_primitives::{Address, B256, U256};
use anyhow::{Context, Result};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use std::path::Path;

/// Trait defining the interface for Ethereum state storage.
///
/// All methods return Results for proper error handling.
/// Missing storage slots return U256::ZERO (Ethereum convention).
pub trait StateStore {
    /// Get an account record by address.
    fn get_account(&self, addr: Address) -> Result<Option<AccountRecord>>;

    /// Store an account record.
    fn put_account(&self, addr: Address, acc: &AccountRecord) -> Result<()>;

    /// Get contract bytecode by code hash.
    fn get_code(&self, code_hash: B256) -> Result<Option<Vec<u8>>>;

    /// Store contract bytecode.
    fn put_code(&self, code_hash: B256, code: &[u8]) -> Result<()>;

    /// Get a storage slot value.
    ///
    /// Returns U256::ZERO if the slot doesn't exist (no error).
    fn get_storage(&self, addr: Address, slot: B256) -> Result<U256>;

    /// Store a storage slot value.
    fn put_storage(&self, addr: Address, slot: B256, val: U256) -> Result<()>;

    /// Get a block header by block number.
    fn get_header(&self, block: u64) -> Result<Option<HeaderRecord>>;

    /// Store a block header.
    fn put_header(&self, block: u64, h: &HeaderRecord) -> Result<()>;

    /// Get a block hash by block number.
    fn get_block_hash(&self, block: u64) -> Result<Option<B256>>;

    /// Store a block hash.
    fn put_block_hash(&self, block: u64, hash: B256) -> Result<()>;

    /// Get the current head block number.
    fn get_head(&self) -> Result<Option<u64>>;

    /// Set the current head block number.
    fn set_head(&self, block: u64) -> Result<()>;
}

/// RocksDB-backed implementation of StateStore.
///
/// Uses column families to organize different types of data:
/// - accounts: account records
/// - code: contract bytecode
/// - storage: storage slot values
/// - headers: block headers
/// - block_hashes: block hashes
/// - meta: metadata (head block, etc.)
pub struct RocksStateStore {
    db: DB,
}

impl RocksStateStore {
    /// Open or create a RocksDB database at the given path.
    ///
    /// Creates all required column families if they don't exist.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // Define all column families
        let column_families = vec![
            ColumnFamilyDescriptor::new("accounts", Options::default()),
            ColumnFamilyDescriptor::new("code", Options::default()),
            ColumnFamilyDescriptor::new("storage", Options::default()),
            ColumnFamilyDescriptor::new("headers", Options::default()),
            ColumnFamilyDescriptor::new("block_hashes", Options::default()),
            ColumnFamilyDescriptor::new("meta", Options::default()),
        ];

        let db = DB::open_cf_descriptors(&opts, path, column_families)
            .context("Failed to open RocksDB database")?;

        Ok(Self { db })
    }

    /// Get a column family handle by name.
    fn get_cf(&self, name: &str) -> Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(name)
            .with_context(|| format!("Column family '{}' not found", name))
    }
}

impl StateStore for RocksStateStore {
    fn get_account(&self, addr: Address) -> Result<Option<AccountRecord>> {
        let cf = self.get_cf("accounts")?;
        let key = encode_account_key(addr);
        match self.db.get_cf(cf, &key).context("Failed to get account")? {
            Some(bytes) => {
                let record = postcard::from_bytes(&bytes)
                    .context("Failed to deserialize account record")?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    fn put_account(&self, addr: Address, acc: &AccountRecord) -> Result<()> {
        let cf = self.get_cf("accounts")?;
        let key = encode_account_key(addr);
        let value = postcard::to_allocvec(acc).context("Failed to serialize account record")?;
        self.db
            .put_cf(cf, &key, &value)
            .context("Failed to put account")?;
        Ok(())
    }

    fn get_code(&self, code_hash: B256) -> Result<Option<Vec<u8>>> {
        let cf = self.get_cf("code")?;
        let key = encode_code_key(code_hash);
        match self.db.get_cf(cf, &key).context("Failed to get code")? {
            Some(bytes) => Ok(Some(bytes)),
            None => Ok(None),
        }
    }

    fn put_code(&self, code_hash: B256, code: &[u8]) -> Result<()> {
        let cf = self.get_cf("code")?;
        let key = encode_code_key(code_hash);
        self.db
            .put_cf(cf, &key, code)
            .context("Failed to put code")?;
        Ok(())
    }

    fn get_storage(&self, addr: Address, slot: B256) -> Result<U256> {
        let cf = self.get_cf("storage")?;
        let key = encode_storage_key(addr, slot);
        match self.db.get_cf(cf, &key).context("Failed to get storage")? {
            Some(bytes) => decode_u256(&bytes).context("Failed to decode storage value"),
            None => Ok(U256::ZERO), // Ethereum convention: missing storage is zero
        }
    }

    fn put_storage(&self, addr: Address, slot: B256, val: U256) -> Result<()> {
        let cf = self.get_cf("storage")?;
        let key = encode_storage_key(addr, slot);
        let value = encode_u256(val);
        self.db
            .put_cf(cf, &key, &value)
            .context("Failed to put storage")?;
        Ok(())
    }

    fn get_header(&self, block: u64) -> Result<Option<HeaderRecord>> {
        let cf = self.get_cf("headers")?;
        let key = encode_header_key(block);
        match self.db.get_cf(cf, &key).context("Failed to get header")? {
            Some(bytes) => {
                let record = postcard::from_bytes(&bytes)
                    .context("Failed to deserialize header record")?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    fn put_header(&self, block: u64, h: &HeaderRecord) -> Result<()> {
        let cf = self.get_cf("headers")?;
        let key = encode_header_key(block);
        let value = postcard::to_allocvec(h).context("Failed to serialize header record")?;
        self.db
            .put_cf(cf, &key, &value)
            .context("Failed to put header")?;
        Ok(())
    }

    fn get_block_hash(&self, block: u64) -> Result<Option<B256>> {
        let cf = self.get_cf("block_hashes")?;
        let key = encode_block_hash_key(block);
        match self.db.get_cf(cf, &key).context("Failed to get block hash")? {
            Some(bytes) => {
                if bytes.len() != 32 {
                    anyhow::bail!("Block hash must be 32 bytes, got {}", bytes.len());
                }
                Ok(Some(B256::from_slice(&bytes)))
            }
            None => Ok(None),
        }
    }

    fn put_block_hash(&self, block: u64, hash: B256) -> Result<()> {
        let cf = self.get_cf("block_hashes")?;
        let key = encode_block_hash_key(block);
        self.db
            .put_cf(cf, &key, hash.as_slice())
            .context("Failed to put block hash")?;
        Ok(())
    }

    fn get_head(&self) -> Result<Option<u64>> {
        let cf = self.get_cf("meta")?;
        let key = encode_meta_key(0x01); // 0x01 = head_block
        match self.db.get_cf(cf, &key).context("Failed to get head block")? {
            Some(bytes) => {
                if bytes.len() != 8 {
                    anyhow::bail!("Head block must be 8 bytes (u64), got {}", bytes.len());
                }
                Ok(Some(u64::from_be_bytes(
                    bytes.try_into().expect("8 bytes for u64"),
                )))
            }
            None => Ok(None),
        }
    }

    fn set_head(&self, block: u64) -> Result<()> {
        let cf = self.get_cf("meta")?;
        let key = encode_meta_key(0x01); // 0x01 = head_block
        let value = block.to_be_bytes();
        self.db
            .put_cf(cf, &key, &value)
            .context("Failed to set head block")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, b256};
    use hex;
    use tempfile::TempDir;

    fn create_test_store() -> (RocksStateStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = RocksStateStore::open(temp_dir.path()).unwrap();
        (store, temp_dir)
    }

    #[test]
    fn test_missing_storage_returns_zero() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let slot = b256!("0000000000000000000000000000000000000000000000000000000000000000");

        let value = store.get_storage(addr, slot).unwrap();
        assert_eq!(value, U256::ZERO);
    }

    #[test]
    fn test_account_roundtrip() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let account = AccountRecord {
            nonce: 42,
            balance: U256::from(1000000000000000000u64), // 1 ETH
            code_hash: b256!("0000000000000000000000000000000000000000000000000000000000000000"),
        };

        // Put and get
        store.put_account(addr, &account).unwrap();
        let retrieved = store.get_account(addr).unwrap().unwrap();
        assert_eq!(account, retrieved);

        // Verify it persists
        let retrieved2 = store.get_account(addr).unwrap().unwrap();
        assert_eq!(account, retrieved2);
    }

    #[test]
    fn test_code_roundtrip() {
        let (store, _temp_dir) = create_test_store();
        let code_hash = b256!("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef");
        let code = vec![0x60, 0x00, 0x60, 0x00, 0x52]; // PUSH1 0 PUSH1 0 MSTORE

        store.put_code(code_hash, &code).unwrap();
        let retrieved = store.get_code(code_hash).unwrap().unwrap();
        assert_eq!(code, retrieved);
    }

    #[test]
    fn test_header_roundtrip() {
        let (store, _temp_dir) = create_test_store();
        let header = HeaderRecord {
            number: 12345,
            timestamp: 1609459200,
            basefee: U256::from(1000000000u64),
            coinbase: Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap()),
            prevrandao: b256!("abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"),
            gas_limit: 30000000,
            chain_id: 1,
        };

        store.put_header(12345, &header).unwrap();
        let retrieved = store.get_header(12345).unwrap().unwrap();
        assert_eq!(header, retrieved);
    }

    #[test]
    fn test_block_hash_roundtrip() {
        let (store, _temp_dir) = create_test_store();
        let block = 67890u64;
        let hash = b256!("fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321");

        store.put_block_hash(block, hash).unwrap();
        let retrieved = store.get_block_hash(block).unwrap().unwrap();
        assert_eq!(hash, retrieved);
    }

    #[test]
    fn test_head_block_set_get() {
        let (store, _temp_dir) = create_test_store();

        // Initially should be None
        assert_eq!(store.get_head().unwrap(), None);

        // Set and get
        store.set_head(12345).unwrap();
        assert_eq!(store.get_head().unwrap(), Some(12345));

        // Update
        store.set_head(67890).unwrap();
        assert_eq!(store.get_head().unwrap(), Some(67890));
    }
}
