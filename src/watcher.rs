//! Main watcher loop
//!
//! Orchestrates polling finalized blocks, processing transactions,
//! and updating the state store for watched addresses.

use crate::apply::{apply_transaction, check_receiver_is_eoa, is_eoa_to_eoa_transfer};
use crate::cache::ContractCache;
use crate::config::load_watchlist;
use crate::records::AccountRecord;
use crate::rpc::RpcClient;
use crate::store::{RocksStateStore, StateStore};
use alloy_primitives::{Address, B256};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;
use tracing::info;

/// Main watcher that monitors and processes Ethereum blocks.
pub struct Watcher {
    store: RocksStateStore,
    rpc: RpcClient,
    cache: ContractCache,
    watchlist: Vec<Address>,
}

impl Watcher {
    /// Create a new watcher.
    pub fn new(store: RocksStateStore, rpc: RpcClient) -> Self {
        Self {
            store,
            rpc,
            cache: ContractCache::new(),
            watchlist: Vec::new(),
        }
    }

    /// Initialize the watcher.
    ///
    /// Loads the watchlist, fetches initial state for all watched addresses,
    /// and sets the head to the current finalized block.
    /// If the database already has a head set, only initializes addresses that don't exist.
    pub async fn initialize(&mut self, watchlist_path: &Path) -> Result<()> {
        info!("Initializing watcher...");

        // Load watchlist
        self.watchlist = load_watchlist(watchlist_path)
            .context("Failed to load watchlist")?;
        info!("Loaded {} addresses to watch", self.watchlist.len());

        // Check if we already have a head block (resuming from existing state)
        let existing_head = self.store.get_head().context("Failed to get head")?;
        
        // Get current block number (use "latest" for consistency with balance fetching)
        // This ensures we're using the same block reference for both balance and head
        let current_block_num = self
            .rpc
            .get_block_number("latest")
            .await
            .context("Failed to get latest block number")?;
        
        if let Some(head) = existing_head {
            info!("Resuming from existing state. Current head: {}, Latest block: {}", head, current_block_num);
            // Don't overwrite existing state, just verify all addresses exist
            for addr in &self.watchlist {
                if self.store.get_account(*addr)?.is_none() {
                    // Address not in DB, initialize it at latest block
                    let balance = self
                        .rpc
                        .get_balance(*addr, "latest")
                        .await
                        .with_context(|| format!("Failed to get balance for {:?}", addr))?;
                    let nonce = self
                        .rpc
                        .get_transaction_count(*addr, "latest")
                        .await
                        .with_context(|| format!("Failed to get transaction count for {:?}", addr))?;
                    
                    let account = AccountRecord {
                        nonce,
                        balance,
                        code_hash: B256::ZERO,
                    };
                    self.store
                        .put_account(*addr, &account)
                        .with_context(|| format!("Failed to store account for {:?}", addr))?;
                    info!("Initialized new address {:?}: balance={:?}, nonce={}", addr, balance, nonce);
                }
            }
        } else {
            // First run: initialize all addresses at current latest block
            // This is a point-in-time snapshot - we only track changes going forward
            info!("First run. Initializing all addresses at block {} (point-in-time snapshot)", current_block_num);
            // Use "latest" to get the most current balance at initialization time
            for addr in &self.watchlist {
                // Fetch balance at latest block (most accurate for initialization)
                let balance = self
                    .rpc
                    .get_balance(*addr, "latest")
                    .await
                    .with_context(|| format!("Failed to get balance for {:?}", addr))?;

                // Fetch nonce at latest block
                let nonce = self
                    .rpc
                    .get_transaction_count(*addr, "latest")
                    .await
                    .with_context(|| format!("Failed to get transaction count for {:?}", addr))?;

                // Store account
                let account = AccountRecord {
                    nonce,
                    balance,
                    code_hash: B256::ZERO, // EOA has no code
                };

                self.store
                    .put_account(*addr, &account)
                    .with_context(|| format!("Failed to store account for {:?}", addr))?;

                info!(
                    "Initialized {:?}: balance={:?}, nonce={} (at block {})",
                    addr, balance, nonce, current_block_num
                );
            }

            // Set head to current block number (the block we initialized from)
            self.store
                .set_head(current_block_num)
                .context("Failed to set head block")?;
            info!("Initialization complete. Head set to block {}", current_block_num);
        }

        Ok(())
    }

    /// Process a range of blocks sequentially.
    ///
    /// Fetches each block, filters relevant transactions, and applies them.
    pub async fn process_block_range(&mut self, from: u64, to: u64) -> Result<()> {
        if from > to {
            return Ok(()); // Nothing to process
        }

        info!("Processing blocks {} to {}", from, to);
        let watchlist_set: HashSet<Address> = self.watchlist.iter().copied().collect();

        for block_num in from..=to {
            // Fetch full block with transactions
            let block_str = format!("0x{:x}", block_num);
            let block = self
                .rpc
                .get_block_by_number(&block_str, true)
                .await
                .with_context(|| format!("Failed to fetch block {}", block_num))?;

            info!(
                "Processing block {} ({} transactions)",
                block_num,
                block.transactions.len()
            );

            // Process each transaction
            for tx in &block.transactions {
                // Check if transaction affects any watched address
                let sender_watched = watchlist_set.contains(&tx.from);
                let receiver_watched = tx.to.map_or(false, |to| watchlist_set.contains(&to));

                if !sender_watched && !receiver_watched {
                    continue; // Skip transactions that don't affect watchlist
                }

                info!(
                    "Found relevant TX {:?} in block {}: from={:?}, to={:?}, value={:?}",
                    tx.hash, block_num, tx.from, tx.to, tx.value
                );

                // IMPORTANT: If sender is watched, we MUST process the transaction
                // to deduct fees and update nonce, even if it's not an EOA→EOA transfer.
                // For receiver-only transactions, we only process EOA→EOA transfers.
                
                let should_process = if sender_watched {
                    // Sender is watched: process ALL transactions (need to deduct fees)
                    true
                } else {
                    // Only receiver is watched: only process EOA→EOA transfers
                    is_eoa_to_eoa_transfer(tx)
                };

                if !should_process {
                    continue;
                }

                // For EOA→EOA transfers, verify receiver is actually an EOA
                if is_eoa_to_eoa_transfer(tx) {
                    if let Some(receiver) = tx.to {
                        let is_eoa = check_receiver_is_eoa(&self.rpc, &mut self.cache, receiver, block_num)
                            .await
                            .context("Failed to check if receiver is EOA")?;

                        if !is_eoa {
                            // Receiver is a contract, but if sender is watched, we still need to process
                            // to deduct fees. If only receiver is watched, skip.
                            if !sender_watched {
                                continue; // Receiver is a contract and not watched, skip
                            }
                            // Sender is watched, so we'll process it but won't credit receiver
                        }
                    }
                }

                // Always fetch receipt (needed for fee calculation and status)
                let receipt = self
                    .rpc
                    .get_transaction_receipt(tx.hash)
                    .await
                    .with_context(|| {
                        format!("Failed to fetch receipt for tx {:?}", tx.hash)
                    })?;

                // Apply the transaction
                apply_transaction(
                    &self.store,
                    &self.rpc,
                    &mut self.cache,
                    tx,
                    &receipt,
                    &block,
                    &watchlist_set,
                )
                .await
                .with_context(|| {
                    format!("Failed to apply transaction {:?}", tx.hash)
                })?;
            }

            // Update head after processing block
            self.store
                .set_head(block_num)
                .context("Failed to update head block")?;

            info!("Completed block {}", block_num);
        }

        Ok(())
    }

    /// Run the main watcher loop.
    ///
    /// Polls for new blocks every 12 seconds and processes them.
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting watcher loop...");

        loop {
            // Get current local head
            let local_head = self
                .store
                .get_head()
                .context("Failed to get local head")?
                .unwrap_or(0);

            // Get current latest block number (use same method as initialization for consistency)
            let latest_head = self
                .rpc
                .get_block_number("latest")
                .await
                .context("Failed to get latest block number")?;

            if local_head < latest_head {
                info!(
                    "New blocks available: local={}, latest={}",
                    local_head, latest_head
                );

                // Process blocks from local_head + 1 to latest_head
                self.process_block_range(local_head + 1, latest_head)
                    .await
                    .context("Failed to process block range")?;
            } else {
                info!("Up to date. Local head: {}, Latest: {}", local_head, latest_head);
            }

            // Wait 12 seconds before next poll
            tokio::time::sleep(tokio::time::Duration::from_secs(12)).await;
        }
    }
}
