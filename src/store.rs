//! StateStore trait and RocksDB implementation
//!
//! Provides a persistent key-value store for Ethereum state data.
//! Uses RocksDB with column families for efficient organization.

use crate::keys::{
    decode_delta_key, decode_erc20_delta_key, decode_erc20_snapshot_key, decode_snapshot_key,
    encode_account_key, encode_block_hash_key, encode_code_key, encode_delta_key,
    encode_erc20_delta_key, encode_erc20_snapshot_key, encode_header_key, encode_meta_key,
    encode_snapshot_key, encode_storage_key, encode_token_watch_meta_key, encode_watch_meta_key,
};
use crate::records::{
    decode_u256, encode_u256, AccountRecord, BalanceSnapshot, BlockDelta, Erc20Delta, Erc20Snapshot,
    HeaderRecord, TokenWatchMeta, WatchMeta,
};
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

    /// Store a block delta for an address.
    fn put_delta(&self, addr: Address, block: u64, delta: &BlockDelta) -> Result<()>;

    /// Get a block delta for an address.
    fn get_delta(&self, addr: Address, block: u64) -> Result<Option<BlockDelta>>;

    /// Get all deltas for an address in a block range.
    fn get_deltas_in_range(
        &self,
        addr: Address,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<(u64, BlockDelta)>>;

    /// Store a balance snapshot for an address at a block.
    fn put_snapshot(&self, addr: Address, block: u64, balance: BalanceSnapshot) -> Result<()>;

    /// Get a balance snapshot for an address at a block.
    fn get_snapshot(&self, addr: Address, block: u64) -> Result<Option<BalanceSnapshot>>;

    /// Get the latest snapshot at or before a given block.
    fn get_latest_snapshot_at_or_before(
        &self,
        addr: Address,
        block: u64,
    ) -> Result<Option<(u64, BalanceSnapshot)>>;

    /// Get balances for an address in a block range using fill-forward logic.
    fn get_balances_in_range(
        &self,
        addr: Address,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<(u64, U256)>>;

    /// Store watch metadata for an address.
    fn put_watch_meta(&self, addr: Address, meta: &WatchMeta) -> Result<()>;

    /// Get watch metadata for an address.
    fn get_watch_meta(&self, addr: Address) -> Result<Option<WatchMeta>>;

    /// Get deltas for an address in a block range with coverage metadata.
    fn get_deltas_in_range_with_metadata(
        &self,
        addr: Address,
        requested_start: u64,
        requested_end: u64,
    ) -> Result<QueryResult<BlockDelta>>;

    /// Get balances for an address in a block range with coverage metadata.
    fn get_balances_in_range_with_metadata(
        &self,
        addr: Address,
        requested_start: u64,
        requested_end: u64,
    ) -> Result<QueryResult<U256>>;

    // ─────────────────────────────────────────────────────────────────
    // ERC20 token tracking
    // ─────────────────────────────────────────────────────────────────

    /// Store an ERC20 delta for (token, owner) at a block.
    fn put_erc20_delta(
        &self,
        token: Address,
        owner: Address,
        block: u64,
        delta: &Erc20Delta,
    ) -> Result<()>;

    /// Get ERC20 deltas for (token, owner) in a block range.
    fn get_erc20_deltas_in_range(
        &self,
        token: Address,
        owner: Address,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<(u64, Erc20Delta)>>;

    /// Store an ERC20 snapshot for (token, owner) at a block.
    fn put_erc20_snapshot(
        &self,
        token: Address,
        owner: Address,
        block: u64,
        balance: Erc20Snapshot,
    ) -> Result<()>;

    /// Get the latest ERC20 snapshot at or before a block.
    fn get_latest_erc20_snapshot_at_or_before(
        &self,
        token: Address,
        owner: Address,
        block: u64,
    ) -> Result<Option<(u64, Erc20Snapshot)>>;

    /// Store current ERC20 balance (for internal tracking).
    fn put_erc20_balance(&self, token: Address, owner: Address, balance: U256) -> Result<()>;

    /// Get current ERC20 balance.
    fn get_erc20_balance(&self, token: Address, owner: Address) -> Result<Option<U256>>;

    /// Store token watch metadata for (token, owner).
    fn put_token_watch_meta(
        &self,
        token: Address,
        owner: Address,
        meta: &TokenWatchMeta,
    ) -> Result<()>;

    /// Get token watch metadata for (token, owner).
    fn get_token_watch_meta(
        &self,
        token: Address,
        owner: Address,
    ) -> Result<Option<TokenWatchMeta>>;

    /// Get ERC20 deltas in range with coverage metadata.
    fn get_erc20_deltas_in_range_with_metadata(
        &self,
        token: Address,
        owner: Address,
        requested_start: u64,
        requested_end: u64,
    ) -> Result<QueryResult<Erc20Delta>>;

    /// Get ERC20 balances in range with coverage metadata (fill-forward).
    fn get_erc20_balances_in_range_with_metadata(
        &self,
        token: Address,
        owner: Address,
        requested_start: u64,
        requested_end: u64,
    ) -> Result<QueryResult<U256>>;
}

/// Query result with coverage metadata.
///
/// Contains information about how the requested range was clamped
/// to respect coverage boundaries (watch_start_block and head_block).
#[derive(Debug, Clone)]
pub struct QueryResult<T> {
    /// The originally requested start block.
    pub requested_start: u64,
    /// The originally requested end block.
    pub requested_end: u64,
    /// The effective start block after clamping.
    pub effective_start: u64,
    /// The effective end block after clamping.
    pub effective_end: u64,
    /// The block at which we started watching this address.
    pub watch_start_block: u64,
    /// The current head block (if available).
    pub head_block: Option<u64>,
    /// Optional message explaining any clamping that occurred.
    pub message: Option<String>,
    /// The actual query results (block, value) pairs.
    pub data: Vec<(u64, T)>,
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
            ColumnFamilyDescriptor::new("block_deltas", Options::default()),
            ColumnFamilyDescriptor::new("balance_snapshots", Options::default()),
            ColumnFamilyDescriptor::new("watch_meta", Options::default()),
            // ERC20 token tracking
            ColumnFamilyDescriptor::new("erc20_deltas", Options::default()),
            ColumnFamilyDescriptor::new("erc20_snapshots", Options::default()),
            ColumnFamilyDescriptor::new("erc20_watch_meta", Options::default()),
            ColumnFamilyDescriptor::new("erc20_balances", Options::default()),
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

    fn put_delta(&self, addr: Address, block: u64, delta: &BlockDelta) -> Result<()> {
        let cf = self.get_cf("block_deltas")?;
        let key = encode_delta_key(addr, block);
        let value = postcard::to_allocvec(delta).context("Failed to serialize delta")?;
        self.db
            .put_cf(cf, &key, &value)
            .context("Failed to put delta")?;
        Ok(())
    }

    fn get_delta(&self, addr: Address, block: u64) -> Result<Option<BlockDelta>> {
        let cf = self.get_cf("block_deltas")?;
        let key = encode_delta_key(addr, block);
        match self.db.get_cf(cf, &key).context("Failed to get delta")? {
            Some(bytes) => {
                let delta = postcard::from_bytes(&bytes)
                    .context("Failed to deserialize delta")?;
                Ok(Some(delta))
            }
            None => Ok(None),
        }
    }

    fn get_deltas_in_range(
        &self,
        addr: Address,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<(u64, BlockDelta)>> {
        let cf = self.get_cf("block_deltas")?;
        let start_key = encode_delta_key(addr, start_block);
        let end_key = encode_delta_key(addr, end_block.saturating_add(1)); // Exclusive end

        let mut deltas = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::From(&start_key, rocksdb::Direction::Forward));

        for item in iter {
            let (key, value) = item.context("Failed to read iterator")?;
            
            // Stop if we've gone past the end key
            if key.as_ref() >= end_key.as_slice() {
                break;
            }

            // Decode the key
            let (key_addr, block) = decode_delta_key(&key)
                .context("Failed to decode delta key")?;

            // Only include deltas for this address (safety check)
            if key_addr != addr {
                continue;
            }

            // Deserialize the delta
            let delta: BlockDelta = postcard::from_bytes(&value)
                .context("Failed to deserialize delta")?;

            deltas.push((block, delta));
        }

        Ok(deltas)
    }

    fn put_snapshot(&self, addr: Address, block: u64, balance: BalanceSnapshot) -> Result<()> {
        let cf = self.get_cf("balance_snapshots")?;
        let key = encode_snapshot_key(addr, block);
        let value = encode_u256(balance);
        self.db
            .put_cf(cf, &key, &value)
            .context("Failed to put snapshot")?;
        Ok(())
    }

    fn get_snapshot(&self, addr: Address, block: u64) -> Result<Option<BalanceSnapshot>> {
        let cf = self.get_cf("balance_snapshots")?;
        let key = encode_snapshot_key(addr, block);
        match self.db.get_cf(cf, &key).context("Failed to get snapshot")? {
            Some(bytes) => {
                let balance = decode_u256(&bytes)
                    .context("Failed to decode snapshot")?;
                Ok(Some(balance))
            }
            None => Ok(None),
        }
    }

    fn get_latest_snapshot_at_or_before(
        &self,
        addr: Address,
        block: u64,
    ) -> Result<Option<(u64, BalanceSnapshot)>> {
        let cf = self.get_cf("balance_snapshots")?;
        
        // Create a key for the maximum possible block for this address
        // We'll iterate backwards from there
        let search_key = encode_snapshot_key(addr, block);
        
        // Use a reverse iterator starting from the search key
        let iter = self.db.iterator_cf(
            cf,
            rocksdb::IteratorMode::From(&search_key, rocksdb::Direction::Reverse),
        );

        for item in iter {
            let (key, value) = item.context("Failed to read iterator")?;
            
            // Decode the key
            let (key_addr, key_block) = decode_snapshot_key(&key)
                .context("Failed to decode snapshot key")?;

            // Only consider snapshots for this address
            if key_addr != addr {
                continue;
            }

            // If this snapshot is at or before the requested block, we found it
            if key_block <= block {
                let balance = decode_u256(&value)
                    .context("Failed to decode snapshot")?;
                return Ok(Some((key_block, balance)));
            }
        }

        Ok(None)
    }

    fn get_balances_in_range(
        &self,
        addr: Address,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<(u64, U256)>> {
        // Use the metadata version and extract just the data
        let result = self.get_balances_in_range_with_metadata(addr, start_block, end_block)?;
        Ok(result.data)
    }

    fn get_balances_in_range_with_metadata(
        &self,
        addr: Address,
        requested_start: u64,
        requested_end: u64,
    ) -> Result<QueryResult<U256>> {
        // Get watch metadata
        let watch_meta = self
            .get_watch_meta(addr)?
            .ok_or_else(|| anyhow::anyhow!("Address {:?} is not being tracked", addr))?;

        // Get head block
        let head_block = self.get_head()?;

        // Clamp start and end blocks
        let effective_start = requested_start.max(watch_meta.start_block);
        let effective_end = if let Some(head) = head_block {
            requested_end.min(head)
        } else {
            requested_end
        };

        // Build message if clamping occurred
        let mut message_parts = Vec::new();
        if effective_start > requested_start {
            message_parts.push(format!(
                "Earliest known balance starts at block {}.",
                watch_meta.start_block
            ));
        }
        if let Some(head) = head_block {
            if effective_end < requested_end {
                message_parts.push(format!("Latest available block is {}.", head));
            }
        }
        let message = if message_parts.is_empty() {
            None
        } else {
            Some(message_parts.join(" "))
        };

        // If effective range is invalid, return empty results
        if effective_start > effective_end {
            return Ok(QueryResult {
                requested_start,
                requested_end,
                effective_start,
                effective_end,
                watch_start_block: watch_meta.start_block,
                head_block,
                message,
                data: Vec::new(),
            });
        }

        // Find the anchor snapshot
        // We require a snapshot at watch_start_block or later, not before
        let anchor = self
            .get_latest_snapshot_at_or_before(addr, effective_start)
            .context("Failed to get anchor snapshot")?;

        let mut balance = match anchor {
            Some((snapshot_block, bal)) => {
                // Reject snapshots from before watch_start_block
                if snapshot_block < watch_meta.start_block {
                    anyhow::bail!(
                        "No snapshot found at or after watch_start_block {} for address {:?}. \
                        Found snapshot at block {} which is before coverage started. \
                        Please reinitialize/backfill snapshots.",
                        watch_meta.start_block, addr, snapshot_block
                    );
                }
                bal
            }
            None => {
                // No snapshot found - we need a snapshot at watch_start_block
                anyhow::bail!(
                    "No snapshot found at watch_start_block {} for address {:?}. \
                    Please reinitialize/backfill snapshots.",
                    watch_meta.start_block, addr
                );
            }
        };

        // Get all deltas in the effective range
        let deltas = self
            .get_deltas_in_range(addr, effective_start, effective_end)
            .context("Failed to get deltas in range")?;

        // Create a map of block -> delta for efficient lookup
        let delta_map: std::collections::HashMap<u64, BlockDelta> = deltas.into_iter().collect();

        // Build the result by iterating through each block in the effective range
        let mut results = Vec::new();
        for block in effective_start..=effective_end {
            // Apply delta if it exists for this block
            if let Some(delta) = delta_map.get(&block) {
                // Apply the delta: balance = balance + delta_plus - delta_minus
                balance = balance
                    .saturating_add(delta.delta_plus)
                    .saturating_sub(delta.delta_minus);
            }
            // If no delta, balance stays the same (fill-forward)

            results.push((block, balance));
        }

        Ok(QueryResult {
            requested_start,
            requested_end,
            effective_start,
            effective_end,
            watch_start_block: watch_meta.start_block,
            head_block,
            message,
            data: results,
        })
    }

    fn put_watch_meta(&self, addr: Address, meta: &WatchMeta) -> Result<()> {
        let cf = self.get_cf("watch_meta")?;
        let key = encode_watch_meta_key(addr);
        let value = postcard::to_allocvec(meta).context("Failed to serialize watch meta")?;
        self.db
            .put_cf(cf, &key, &value)
            .context("Failed to put watch meta")?;
        Ok(())
    }

    fn get_watch_meta(&self, addr: Address) -> Result<Option<WatchMeta>> {
        let cf = self.get_cf("watch_meta")?;
        let key = encode_watch_meta_key(addr);
        match self.db.get_cf(cf, &key).context("Failed to get watch meta")? {
            Some(bytes) => {
                let meta = postcard::from_bytes(&bytes)
                    .context("Failed to deserialize watch meta")?;
                Ok(Some(meta))
            }
            None => Ok(None),
        }
    }

    fn get_deltas_in_range_with_metadata(
        &self,
        addr: Address,
        requested_start: u64,
        requested_end: u64,
    ) -> Result<QueryResult<BlockDelta>> {
        // Get watch metadata
        let watch_meta = self
            .get_watch_meta(addr)?
            .ok_or_else(|| anyhow::anyhow!("Address {:?} is not being tracked", addr))?;

        // Get head block
        let head_block = self.get_head()?;

        // Clamp start and end blocks
        let effective_start = requested_start.max(watch_meta.start_block);
        let effective_end = if let Some(head) = head_block {
            requested_end.min(head)
        } else {
            requested_end
        };

        // Build message if clamping occurred
        let mut message_parts = Vec::new();
        if effective_start > requested_start {
            message_parts.push(format!(
                "Earliest known balance starts at block {}.",
                watch_meta.start_block
            ));
        }
        if let Some(head) = head_block {
            if effective_end < requested_end {
                message_parts.push(format!("Latest available block is {}.", head));
            }
        }
        let message = if message_parts.is_empty() {
            None
        } else {
            Some(message_parts.join(" "))
        };

        // If effective range is invalid, return empty results
        if effective_start > effective_end {
            return Ok(QueryResult {
                requested_start,
                requested_end,
                effective_start,
                effective_end,
                watch_start_block: watch_meta.start_block,
                head_block,
                message,
                data: Vec::new(),
            });
        }

        // Get deltas in the effective range
        let cf = self.get_cf("block_deltas")?;
        let start_key = encode_delta_key(addr, effective_start);
        let end_key = encode_delta_key(addr, effective_end.saturating_add(1)); // Exclusive end

        let mut deltas = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::From(&start_key, rocksdb::Direction::Forward));

        for item in iter {
            let (key, value) = item.context("Failed to read iterator")?;
            
            // Stop if we've gone past the end key
            if key.as_ref() >= end_key.as_slice() {
                break;
            }

            // Decode the key
            let (key_addr, block) = decode_delta_key(&key)
                .context("Failed to decode delta key")?;

            // Only include deltas for this address (safety check)
            if key_addr != addr {
                continue;
            }

            // Deserialize the delta
            let delta: BlockDelta = postcard::from_bytes(&value)
                .context("Failed to deserialize delta")?;

            deltas.push((block, delta));
        }

        Ok(QueryResult {
            requested_start,
            requested_end,
            effective_start,
            effective_end,
            watch_start_block: watch_meta.start_block,
            head_block,
            message,
            data: deltas,
        })
    }

    // ─────────────────────────────────────────────────────────────────
    // ERC20 implementations
    // ─────────────────────────────────────────────────────────────────

    fn put_erc20_delta(
        &self,
        token: Address,
        owner: Address,
        block: u64,
        delta: &Erc20Delta,
    ) -> Result<()> {
        let cf = self.get_cf("erc20_deltas")?;
        let key = encode_erc20_delta_key(token, owner, block);
        let value = postcard::to_allocvec(delta).context("Failed to serialize ERC20 delta")?;
        self.db
            .put_cf(cf, &key, &value)
            .context("Failed to put ERC20 delta")?;
        Ok(())
    }

    fn get_erc20_deltas_in_range(
        &self,
        token: Address,
        owner: Address,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<(u64, Erc20Delta)>> {
        let cf = self.get_cf("erc20_deltas")?;
        let start_key = encode_erc20_delta_key(token, owner, start_block);
        let end_key = encode_erc20_delta_key(token, owner, end_block.saturating_add(1));

        let mut deltas = Vec::new();
        let iter = self.db.iterator_cf(
            cf,
            rocksdb::IteratorMode::From(&start_key, rocksdb::Direction::Forward),
        );

        for item in iter {
            let (key, value) = item.context("Failed to read iterator")?;
            if key.as_ref() >= end_key.as_slice() {
                break;
            }
            let (k_token, k_owner, block) =
                decode_erc20_delta_key(&key).context("Failed to decode ERC20 delta key")?;
            if k_token != token || k_owner != owner {
                continue;
            }
            let delta: Erc20Delta =
                postcard::from_bytes(&value).context("Failed to deserialize ERC20 delta")?;
            deltas.push((block, delta));
        }
        Ok(deltas)
    }

    fn put_erc20_snapshot(
        &self,
        token: Address,
        owner: Address,
        block: u64,
        balance: Erc20Snapshot,
    ) -> Result<()> {
        let cf = self.get_cf("erc20_snapshots")?;
        let key = encode_erc20_snapshot_key(token, owner, block);
        let value = encode_u256(balance);
        self.db
            .put_cf(cf, &key, &value)
            .context("Failed to put ERC20 snapshot")?;
        Ok(())
    }

    fn get_latest_erc20_snapshot_at_or_before(
        &self,
        token: Address,
        owner: Address,
        block: u64,
    ) -> Result<Option<(u64, Erc20Snapshot)>> {
        let cf = self.get_cf("erc20_snapshots")?;
        let search_key = encode_erc20_snapshot_key(token, owner, block);
        let iter = self.db.iterator_cf(
            cf,
            rocksdb::IteratorMode::From(&search_key, rocksdb::Direction::Reverse),
        );

        for item in iter {
            let (key, value) = item.context("Failed to read iterator")?;
            let (k_token, k_owner, key_block) =
                decode_erc20_snapshot_key(&key).context("Failed to decode ERC20 snapshot key")?;
            if k_token != token || k_owner != owner {
                continue;
            }
            if key_block <= block {
                let balance =
                    decode_u256(&value).context("Failed to decode ERC20 snapshot")?;
                return Ok(Some((key_block, balance)));
            }
        }
        Ok(None)
    }

    fn put_erc20_balance(&self, token: Address, owner: Address, balance: U256) -> Result<()> {
        let cf = self.get_cf("erc20_balances")?;
        let key = encode_token_watch_meta_key(token, owner); // Reuse same key layout: token+owner
        let value = encode_u256(balance);
        self.db
            .put_cf(cf, &key, &value)
            .context("Failed to put ERC20 balance")?;
        Ok(())
    }

    fn get_erc20_balance(&self, token: Address, owner: Address) -> Result<Option<U256>> {
        let cf = self.get_cf("erc20_balances")?;
        let key = encode_token_watch_meta_key(token, owner);
        match self.db.get_cf(cf, &key).context("Failed to get ERC20 balance")? {
            Some(bytes) => {
                let balance = decode_u256(&bytes).context("Failed to decode ERC20 balance")?;
                Ok(Some(balance))
            }
            None => Ok(None),
        }
    }

    fn put_token_watch_meta(
        &self,
        token: Address,
        owner: Address,
        meta: &TokenWatchMeta,
    ) -> Result<()> {
        let cf = self.get_cf("erc20_watch_meta")?;
        let key = encode_token_watch_meta_key(token, owner);
        let value =
            postcard::to_allocvec(meta).context("Failed to serialize token watch meta")?;
        self.db
            .put_cf(cf, &key, &value)
            .context("Failed to put token watch meta")?;
        Ok(())
    }

    fn get_token_watch_meta(
        &self,
        token: Address,
        owner: Address,
    ) -> Result<Option<TokenWatchMeta>> {
        let cf = self.get_cf("erc20_watch_meta")?;
        let key = encode_token_watch_meta_key(token, owner);
        match self.db.get_cf(cf, &key).context("Failed to get token watch meta")? {
            Some(bytes) => {
                let meta =
                    postcard::from_bytes(&bytes).context("Failed to deserialize token watch meta")?;
                Ok(Some(meta))
            }
            None => Ok(None),
        }
    }

    fn get_erc20_deltas_in_range_with_metadata(
        &self,
        token: Address,
        owner: Address,
        requested_start: u64,
        requested_end: u64,
    ) -> Result<QueryResult<Erc20Delta>> {
        let watch_meta = self
            .get_token_watch_meta(token, owner)?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Token {:?} for owner {:?} is not being tracked",
                    token,
                    owner
                )
            })?;

        let head_block = self.get_head()?;
        let effective_start = requested_start.max(watch_meta.start_block);
        let effective_end = if let Some(head) = head_block {
            requested_end.min(head)
        } else {
            requested_end
        };

        let mut message_parts = Vec::new();
        if effective_start > requested_start {
            message_parts.push(format!(
                "Earliest known token balance starts at block {}.",
                watch_meta.start_block
            ));
        }
        if let Some(head) = head_block {
            if effective_end < requested_end {
                message_parts.push(format!("Latest available block is {}.", head));
            }
        }
        let message = if message_parts.is_empty() {
            None
        } else {
            Some(message_parts.join(" "))
        };

        if effective_start > effective_end {
            return Ok(QueryResult {
                requested_start,
                requested_end,
                effective_start,
                effective_end,
                watch_start_block: watch_meta.start_block,
                head_block,
                message,
                data: Vec::new(),
            });
        }

        let deltas = self.get_erc20_deltas_in_range(token, owner, effective_start, effective_end)?;

        Ok(QueryResult {
            requested_start,
            requested_end,
            effective_start,
            effective_end,
            watch_start_block: watch_meta.start_block,
            head_block,
            message,
            data: deltas,
        })
    }

    fn get_erc20_balances_in_range_with_metadata(
        &self,
        token: Address,
        owner: Address,
        requested_start: u64,
        requested_end: u64,
    ) -> Result<QueryResult<U256>> {
        let watch_meta = self
            .get_token_watch_meta(token, owner)?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Token {:?} for owner {:?} is not being tracked",
                    token,
                    owner
                )
            })?;

        let head_block = self.get_head()?;
        let effective_start = requested_start.max(watch_meta.start_block);
        let effective_end = if let Some(head) = head_block {
            requested_end.min(head)
        } else {
            requested_end
        };

        let mut message_parts = Vec::new();
        if effective_start > requested_start {
            message_parts.push(format!(
                "Earliest known token balance starts at block {}.",
                watch_meta.start_block
            ));
        }
        if let Some(head) = head_block {
            if effective_end < requested_end {
                message_parts.push(format!("Latest available block is {}.", head));
            }
        }
        let message = if message_parts.is_empty() {
            None
        } else {
            Some(message_parts.join(" "))
        };

        if effective_start > effective_end {
            return Ok(QueryResult {
                requested_start,
                requested_end,
                effective_start,
                effective_end,
                watch_start_block: watch_meta.start_block,
                head_block,
                message,
                data: Vec::new(),
            });
        }

        let anchor = self
            .get_latest_erc20_snapshot_at_or_before(token, owner, effective_start)
            .context("Failed to get anchor ERC20 snapshot")?;

        let mut balance = match anchor {
            Some((snapshot_block, bal)) => {
                if snapshot_block < watch_meta.start_block {
                    anyhow::bail!(
                        "No snapshot found at or after watch_start_block {} for token {:?} owner {:?}. \
                        Found snapshot at block {} which is before coverage started. \
                        Please reinitialize/backfill snapshots.",
                        watch_meta.start_block, token, owner, snapshot_block
                    );
                }
                bal
            }
            None => {
                anyhow::bail!(
                    "No snapshot found at watch_start_block {} for token {:?} owner {:?}. \
                    Please reinitialize/backfill snapshots.",
                    watch_meta.start_block, token, owner
                );
            }
        };

        let deltas =
            self.get_erc20_deltas_in_range(token, owner, effective_start, effective_end)?;
        let delta_map: std::collections::HashMap<u64, Erc20Delta> =
            deltas.into_iter().collect();

        let mut results = Vec::new();
        for block in effective_start..=effective_end {
            if let Some(delta) = delta_map.get(&block) {
                balance = balance
                    .saturating_add(delta.delta_plus)
                    .saturating_sub(delta.delta_minus);
            }
            results.push((block, balance));
        }

        Ok(QueryResult {
            requested_start,
            requested_end,
            effective_start,
            effective_end,
            watch_start_block: watch_meta.start_block,
            head_block,
            message,
            data: results,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::Erc20Delta;
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

    #[test]
    fn test_delta_storage() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let block = 100u64;

        let delta = BlockDelta {
            block: 100,
            delta_plus: U256::from(1000u64),
            delta_minus: U256::from(500u64),
            received_value: U256::from(1000u64),
            sent_value: U256::from(300u64),
            fee_paid: U256::from(200u64),
            failed_fee: U256::ZERO,
            nonce_delta: 1,
            tx_count: 1,
        };

        // Store and retrieve
        store.put_delta(addr, block, &delta).unwrap();
        let retrieved = store.get_delta(addr, block).unwrap().unwrap();
        assert_eq!(delta, retrieved);
    }

    #[test]
    fn test_snapshot_storage() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let block = 100u64;
        let balance = U256::from(1000000u64);

        // Store and retrieve
        store.put_snapshot(addr, block, balance).unwrap();
        let retrieved = store.get_snapshot(addr, block).unwrap().unwrap();
        assert_eq!(balance, retrieved);
    }

    #[test]
    fn test_latest_snapshot_at_or_before() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        // Store snapshots at blocks 100, 105, 110
        store.put_snapshot(addr, 100, U256::from(1000u64)).unwrap();
        store.put_snapshot(addr, 105, U256::from(2000u64)).unwrap();
        store.put_snapshot(addr, 110, U256::from(3000u64)).unwrap();

        // Exact match
        let result = store.get_latest_snapshot_at_or_before(addr, 105).unwrap();
        assert_eq!(result, Some((105, U256::from(2000u64))));

        // Between snapshots (should return earlier one)
        let result = store.get_latest_snapshot_at_or_before(addr, 107).unwrap();
        assert_eq!(result, Some((105, U256::from(2000u64))));

        // Before any snapshot
        let result = store.get_latest_snapshot_at_or_before(addr, 50).unwrap();
        assert_eq!(result, None);

        // At or after last snapshot
        let result = store.get_latest_snapshot_at_or_before(addr, 110).unwrap();
        assert_eq!(result, Some((110, U256::from(3000u64))));

        let result = store.get_latest_snapshot_at_or_before(addr, 200).unwrap();
        assert_eq!(result, Some((110, U256::from(3000u64))));
    }

    #[test]
    fn test_get_deltas_in_range() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        // Store deltas at blocks 101, 103, 105
        let delta1 = BlockDelta {
            block: 101,
            delta_plus: U256::ZERO,
            delta_minus: U256::from(100u64),
            received_value: U256::ZERO,
            sent_value: U256::from(80u64),
            fee_paid: U256::from(20u64),
            failed_fee: U256::ZERO,
            nonce_delta: 1,
            tx_count: 1,
        };
        let delta2 = BlockDelta {
            block: 103,
            delta_plus: U256::from(500u64),
            delta_minus: U256::ZERO,
            received_value: U256::from(500u64),
            sent_value: U256::ZERO,
            fee_paid: U256::ZERO,
            failed_fee: U256::ZERO,
            nonce_delta: 0,
            tx_count: 1,
        };
        let delta3 = BlockDelta {
            block: 105,
            delta_plus: U256::ZERO,
            delta_minus: U256::from(200u64),
            received_value: U256::ZERO,
            sent_value: U256::from(150u64),
            fee_paid: U256::from(50u64),
            failed_fee: U256::ZERO,
            nonce_delta: 1,
            tx_count: 1,
        };

        store.put_delta(addr, 101, &delta1).unwrap();
        store.put_delta(addr, 103, &delta2).unwrap();
        store.put_delta(addr, 105, &delta3).unwrap();

        // Query range 101-105
        let deltas = store.get_deltas_in_range(addr, 101, 105).unwrap();
        assert_eq!(deltas.len(), 3);
        assert_eq!(deltas[0].0, 101);
        assert_eq!(deltas[1].0, 103);
        assert_eq!(deltas[2].0, 105);

        // Query range 102-104 (should only get 103)
        let deltas = store.get_deltas_in_range(addr, 102, 104).unwrap();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].0, 103);
    }

    #[test]
    fn test_get_balances_in_range_fill_forward() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        // Set up: watch_start_block = 100, snapshot at block 100 with balance 10000
        let watch_meta = WatchMeta { start_block: 100 };
        store.put_watch_meta(addr, &watch_meta).unwrap();
        store.put_snapshot(addr, 100, U256::from(10000u64)).unwrap();

        // Deltas: block 101 (-100), block 103 (+500), block 105 (-200)
        let delta1 = BlockDelta {
            block: 101,
            delta_plus: U256::ZERO,
            delta_minus: U256::from(100u64),
            received_value: U256::ZERO,
            sent_value: U256::from(80u64),
            fee_paid: U256::from(20u64),
            failed_fee: U256::ZERO,
            nonce_delta: 1,
            tx_count: 1,
        };
        let delta2 = BlockDelta {
            block: 103,
            delta_plus: U256::from(500u64),
            delta_minus: U256::ZERO,
            received_value: U256::from(500u64),
            sent_value: U256::ZERO,
            fee_paid: U256::ZERO,
            failed_fee: U256::ZERO,
            nonce_delta: 0,
            tx_count: 1,
        };
        let delta3 = BlockDelta {
            block: 105,
            delta_plus: U256::ZERO,
            delta_minus: U256::from(200u64),
            received_value: U256::ZERO,
            sent_value: U256::from(150u64),
            fee_paid: U256::from(50u64),
            failed_fee: U256::ZERO,
            nonce_delta: 1,
            tx_count: 1,
        };

        store.put_delta(addr, 101, &delta1).unwrap();
        store.put_delta(addr, 103, &delta2).unwrap();
        store.put_delta(addr, 105, &delta3).unwrap();

        // Query balances from 101 to 105
        let balances = store.get_balances_in_range(addr, 101, 105).unwrap();

        // Expected:
        // 101: 10000 - 100 = 9900
        // 102: 9900 (no delta, fill forward)
        // 103: 9900 + 500 = 10400
        // 104: 10400 (no delta, fill forward)
        // 105: 10400 - 200 = 10200

        assert_eq!(balances.len(), 5);
        assert_eq!(balances[0], (101, U256::from(9900u64)));
        assert_eq!(balances[1], (102, U256::from(9900u64)));
        assert_eq!(balances[2], (103, U256::from(10400u64)));
        assert_eq!(balances[3], (104, U256::from(10400u64)));
        assert_eq!(balances[4], (105, U256::from(10200u64)));
    }

    #[test]
    fn test_no_change_no_storage() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        // Create a delta with no changes
        let empty_delta = BlockDelta::new(100);
        assert!(!empty_delta.has_changes());

        // Even if we try to store it, verify it's truly empty
        // (In practice, we won't store empty deltas, but test the has_changes logic)
        assert_eq!(empty_delta.delta_plus, U256::ZERO);
        assert_eq!(empty_delta.delta_minus, U256::ZERO);
        assert_eq!(empty_delta.nonce_delta, 0);
    }

    #[test]
    fn test_watch_meta_storage() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        let watch_meta = WatchMeta { start_block: 100 };
        store.put_watch_meta(addr, &watch_meta).unwrap();
        let retrieved = store.get_watch_meta(addr).unwrap().unwrap();
        assert_eq!(watch_meta, retrieved);
    }

    #[test]
    fn test_query_start_clamping() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        // Set watch_start_block = 100
        let watch_meta = WatchMeta { start_block: 100 };
        store.put_watch_meta(addr, &watch_meta).unwrap();

        // Create snapshot at block 100
        store.put_snapshot(addr, 100, U256::from(10000u64)).unwrap();

        // Query balances 90..105
        let result = store.get_balances_in_range_with_metadata(addr, 90, 105).unwrap();
        assert_eq!(result.requested_start, 90);
        assert_eq!(result.effective_start, 100);
        assert_eq!(result.effective_end, 105);
        assert_eq!(result.watch_start_block, 100);
        assert_eq!(result.data.len(), 6); // blocks 100..105
        assert!(result.message.is_some());
        assert!(result.message.unwrap().contains("Earliest known balance starts at block 100"));
    }

    #[test]
    fn test_query_end_clamping() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        // Set watch_start_block = 100, head_block = 150
        let watch_meta = WatchMeta { start_block: 100 };
        store.put_watch_meta(addr, &watch_meta).unwrap();
        store.set_head(150).unwrap();

        // Create snapshot at block 100
        store.put_snapshot(addr, 100, U256::from(10000u64)).unwrap();

        // Query balances 100..200
        let result = store.get_balances_in_range_with_metadata(addr, 100, 200).unwrap();
        assert_eq!(result.requested_end, 200);
        assert_eq!(result.effective_end, 150);
        assert_eq!(result.head_block, Some(150));
        assert_eq!(result.data.len(), 51); // blocks 100..150
        assert!(result.message.is_some());
        assert!(result.message.unwrap().contains("Latest available block is 150"));
        
        // Verify no blocks > 150 in results
        for (block, _) in &result.data {
            assert!(*block <= 150);
        }
    }

    #[test]
    fn test_query_both_start_and_end_clamping() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        // Set watch_start_block = 100, head_block = 150
        let watch_meta = WatchMeta { start_block: 100 };
        store.put_watch_meta(addr, &watch_meta).unwrap();
        store.set_head(150).unwrap();

        // Create snapshot at block 100
        store.put_snapshot(addr, 100, U256::from(10000u64)).unwrap();

        // Query balances 90..200
        let result = store.get_balances_in_range_with_metadata(addr, 90, 200).unwrap();
        assert_eq!(result.requested_start, 90);
        assert_eq!(result.requested_end, 200);
        assert_eq!(result.effective_start, 100);
        assert_eq!(result.effective_end, 150);
        assert_eq!(result.data.len(), 51); // blocks 100..150
        assert!(result.message.is_some());
        let msg = result.message.unwrap();
        assert!(msg.contains("Earliest known balance starts at block 100"));
        assert!(msg.contains("Latest available block is 150"));
    }

    #[test]
    fn test_anchor_protection() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        // Create snapshot at block 50
        store.put_snapshot(addr, 50, U256::from(5000u64)).unwrap();

        // Set watch_start_block = 100
        let watch_meta = WatchMeta { start_block: 100 };
        store.put_watch_meta(addr, &watch_meta).unwrap();

        // Query starting at 90 (will be clamped to 100)
        // Should error because snapshot at 50 is before watch_start_block
        let result = store.get_balances_in_range_with_metadata(addr, 90, 105);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("No snapshot found at or after watch_start_block"));
    }

    #[test]
    fn test_error_missing_snapshot_at_watch_start() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        // Set watch_start_block = 100
        let watch_meta = WatchMeta { start_block: 100 };
        store.put_watch_meta(addr, &watch_meta).unwrap();

        // Do NOT create snapshot at 100
        // Query should error
        let result = store.get_balances_in_range_with_metadata(addr, 100, 105);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("No snapshot found at watch_start_block"));
        assert!(err_msg.contains("Please reinitialize/backfill"));
    }

    #[test]
    fn test_deltas_dense_output_respects_coverage() {
        let (store, _temp_dir) = create_test_store();
        let addr = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        // Set watch_start_block = 100, head_block = 150
        let watch_meta = WatchMeta { start_block: 100 };
        store.put_watch_meta(addr, &watch_meta).unwrap();
        store.set_head(150).unwrap();

        // Create deltas at blocks 95, 101, 103, 155
        let delta1 = BlockDelta {
            block: 95,
            delta_plus: U256::ZERO,
            delta_minus: U256::from(100u64),
            received_value: U256::ZERO,
            sent_value: U256::from(80u64),
            fee_paid: U256::from(20u64),
            failed_fee: U256::ZERO,
            nonce_delta: 1,
            tx_count: 1,
        };
        let delta2 = BlockDelta {
            block: 101,
            delta_plus: U256::from(500u64),
            delta_minus: U256::ZERO,
            received_value: U256::from(500u64),
            sent_value: U256::ZERO,
            fee_paid: U256::ZERO,
            failed_fee: U256::ZERO,
            nonce_delta: 0,
            tx_count: 1,
        };
        let delta3 = BlockDelta {
            block: 103,
            delta_plus: U256::ZERO,
            delta_minus: U256::from(200u64),
            received_value: U256::ZERO,
            sent_value: U256::from(150u64),
            fee_paid: U256::from(50u64),
            failed_fee: U256::ZERO,
            nonce_delta: 1,
            tx_count: 1,
        };
        let delta4 = BlockDelta {
            block: 155,
            delta_plus: U256::from(1000u64),
            delta_minus: U256::ZERO,
            received_value: U256::from(1000u64),
            sent_value: U256::ZERO,
            fee_paid: U256::ZERO,
            failed_fee: U256::ZERO,
            nonce_delta: 0,
            tx_count: 1,
        };

        store.put_delta(addr, 95, &delta1).unwrap();
        store.put_delta(addr, 101, &delta2).unwrap();
        store.put_delta(addr, 103, &delta3).unwrap();
        store.put_delta(addr, 155, &delta4).unwrap();

        // Query deltas 90..200 with metadata
        let result = store.get_deltas_in_range_with_metadata(addr, 90, 200).unwrap();

        // Verify only blocks 100..150 are in output
        assert_eq!(result.effective_start, 100);
        assert_eq!(result.effective_end, 150);
        
        // Verify block 95 and 155 are NOT included
        for (block, _) in &result.data {
            assert!(*block >= 100 && *block <= 150, "Block {} should be in range 100..150", block);
        }

        // Verify blocks 101 and 103 are included
        let blocks: Vec<u64> = result.data.iter().map(|(b, _)| *b).collect();
        assert!(blocks.contains(&101));
        assert!(blocks.contains(&103));
        assert!(!blocks.contains(&95));
        assert!(!blocks.contains(&155));
    }

    #[test]
    fn test_erc20_token_watch_meta_storage() {
        let (store, _temp_dir) = create_test_store();
        let token = Address::from_slice(&hex::decode("dAC17F958D2ee523a2206206994597C13D831ec7").unwrap());
        let owner = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());
        let meta = TokenWatchMeta { start_block: 100 };
        store.put_token_watch_meta(token, owner, &meta).unwrap();
        let retrieved = store.get_token_watch_meta(token, owner).unwrap().unwrap();
        assert_eq!(meta, retrieved);
    }

    #[test]
    fn test_erc20_balances_in_range_fill_forward() {
        let (store, _temp_dir) = create_test_store();
        let token = Address::from_slice(&hex::decode("dAC17F958D2ee523a2206206994597C13D831ec7").unwrap());
        let owner = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        let meta = TokenWatchMeta { start_block: 100 };
        store.put_token_watch_meta(token, owner, &meta).unwrap();
        store.put_erc20_snapshot(token, owner, 100, U256::from(10000u64)).unwrap();

        let delta1 = Erc20Delta { block: 101, delta_plus: U256::ZERO, delta_minus: U256::from(100u64), tx_count: 1 };
        let delta2 = Erc20Delta { block: 103, delta_plus: U256::from(500u64), delta_minus: U256::ZERO, tx_count: 1 };
        let delta3 = Erc20Delta { block: 105, delta_plus: U256::ZERO, delta_minus: U256::from(200u64), tx_count: 1 };

        store.put_erc20_delta(token, owner, 101, &delta1).unwrap();
        store.put_erc20_delta(token, owner, 103, &delta2).unwrap();
        store.put_erc20_delta(token, owner, 105, &delta3).unwrap();

        let result = store.get_erc20_balances_in_range_with_metadata(token, owner, 101, 105).unwrap();
        assert_eq!(result.effective_start, 101);
        assert_eq!(result.effective_end, 105);
        assert_eq!(result.data.len(), 5);
        assert_eq!(result.data[0], (101, U256::from(9900u64)));
        assert_eq!(result.data[1], (102, U256::from(9900u64)));
        assert_eq!(result.data[2], (103, U256::from(10400u64)));
        assert_eq!(result.data[3], (104, U256::from(10400u64)));
        assert_eq!(result.data[4], (105, U256::from(10200u64)));
    }

    #[test]
    fn test_erc20_query_coverage_clamping() {
        let (store, _temp_dir) = create_test_store();
        let token = Address::from_slice(&hex::decode("dAC17F958D2ee523a2206206994597C13D831ec7").unwrap());
        let owner = Address::from_slice(&hex::decode("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap());

        let meta = TokenWatchMeta { start_block: 100 };
        store.put_token_watch_meta(token, owner, &meta).unwrap();
        store.put_erc20_snapshot(token, owner, 100, U256::from(10000u64)).unwrap();
        store.set_head(150).unwrap();

        let result = store.get_erc20_balances_in_range_with_metadata(token, owner, 90, 200).unwrap();
        assert_eq!(result.requested_start, 90);
        assert_eq!(result.requested_end, 200);
        assert_eq!(result.effective_start, 100);
        assert_eq!(result.effective_end, 150);
        assert_eq!(result.data.len(), 51);
    }
}
