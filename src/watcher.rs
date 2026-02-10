//! Main watcher loop
//!
//! Orchestrates polling finalized blocks, processing transactions,
//! and updating the state store for watched addresses.

use crate::apply::{apply_internal_credit, apply_transaction, check_receiver_is_eoa, is_eoa_to_eoa_transfer};
use crate::cache::ContractCache;
use crate::config::{load_token_watchlist, load_watchlist};
use crate::records::{AccountRecord, BlockDelta, TokenWatchMeta};
use crate::trace::{collect_internal_transfers, collect_senders};
use crate::tracker::{Tracker, TrackerContext};
use crate::tracker_erc20::Erc20Tracker;
use crate::rpc::RpcClient;
use crate::store::{RocksStateStore, StateStore};
use alloy_primitives::{Address, B256};
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::info;

/// Main watcher that monitors and processes Ethereum blocks.
pub struct Watcher {
    store: RocksStateStore,
    rpc: RpcClient,
    cache: ContractCache,
    watchlist: Vec<Address>,
    /// Watched ERC20 token contract addresses (empty = no ERC20 tracking)
    token_watchlist: Vec<Address>,
    /// ERC20 tracker (used when token_watchlist is non-empty)
    erc20_tracker: Erc20Tracker,
    /// Per-block delta accumulator: address -> BlockDelta
    /// Accumulates changes for the current block being processed
    block_deltas: HashMap<Address, BlockDelta>,
}

impl Watcher {
    /// Create a new watcher.
    pub fn new(store: RocksStateStore, rpc: RpcClient) -> Self {
        Self {
            store,
            rpc,
            cache: ContractCache::new(),
            watchlist: Vec::new(),
            token_watchlist: Vec::new(),
            erc20_tracker: Erc20Tracker::new(Vec::new()),
            block_deltas: HashMap::new(),
        }
    }

    /// Ensure we have contract / EOA information for the given addresses.
    ///
    /// This populates the `ContractCache` by calling `eth_getCode` only for
    /// addresses that are not already cached.
    async fn preload_contract_flags(
        &mut self,
        addrs: &HashSet<Address>,
        block_num: u64,
    ) -> Result<()> {
        for addr in addrs {
            if self.cache.is_contract(*addr).is_some() {
                continue;
            }

            let code = self
                .rpc
                .get_code(*addr, &format!("0x{:x}", block_num))
                .await
                .with_context(|| format!("Failed to get code for address {:?}", addr))?;

            let is_contract = !code.is_empty();
            self.cache.mark_contract(*addr, is_contract);
        }
        Ok(())
    }

    /// Initialize the watcher.
    ///
    /// Loads the watchlist, fetches initial state for all watched addresses,
    /// and sets the head to the current finalized block.
    /// If `tokens_path` is provided and the file exists, also initializes ERC20
    /// tracking for (token, owner) pairs via balanceOf.
    pub async fn initialize(
        &mut self,
        watchlist_path: &Path,
        tokens_path: Option<&Path>,
    ) -> Result<()> {
        info!("Initializing watcher...");

        // Load watchlist
        self.watchlist = load_watchlist(watchlist_path)
            .context("Failed to load watchlist")?;
        info!("Loaded {} addresses to watch", self.watchlist.len());

        // Load token watchlist (optional)
        if let Some(p) = tokens_path {
            if p.exists() {
                self.token_watchlist = load_token_watchlist(p)
                    .context("Failed to load token watchlist")?;
                self.erc20_tracker = Erc20Tracker::new(self.token_watchlist.clone());
                info!("Loaded {} tokens to watch", self.token_watchlist.len());
            }
        }

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

                    // Write snapshot at initialization block
                    self.store
                        .put_snapshot(*addr, current_block_num, balance)
                        .with_context(|| format!("Failed to store initial snapshot for {:?}", addr))?;

                    // Write WatchMeta
                    let watch_meta = crate::records::WatchMeta {
                        start_block: current_block_num,
                    };
                    self.store
                        .put_watch_meta(*addr, &watch_meta)
                        .with_context(|| format!("Failed to store watch metadata for {:?}", addr))?;

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

                // Write snapshot at initialization block
                self.store
                    .put_snapshot(*addr, current_block_num, balance)
                    .with_context(|| format!("Failed to store initial snapshot for {:?}", addr))?;

                // Write WatchMeta
                let watch_meta = crate::records::WatchMeta {
                    start_block: current_block_num,
                };
                self.store
                    .put_watch_meta(*addr, &watch_meta)
                    .with_context(|| format!("Failed to store watch metadata for {:?}", addr))?;

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

        // Initialize ERC20 tracking for (token, owner) pairs if tokens are configured
        if !self.token_watchlist.is_empty() {
            let block_str = format!("0x{:x}", current_block_num);
            for token in &self.token_watchlist {
                for owner in &self.watchlist {
                    // Skip if already initialized (resuming)
                    if self.store.get_token_watch_meta(*token, *owner)?.is_some() {
                        continue;
                    }
                    let balance = self
                        .rpc
                        .erc20_balance_of(*token, *owner, &block_str)
                        .await
                        .with_context(|| {
                            format!(
                                "Failed to get ERC20 balance for token {:?} owner {:?}",
                                token, owner
                            )
                        })?;
                    self.store
                        .put_erc20_balance(*token, *owner, balance)
                        .context("Failed to store ERC20 balance")?;
                    self.store
                        .put_erc20_snapshot(*token, *owner, current_block_num, balance)
                        .context("Failed to store ERC20 snapshot")?;
                    self.store
                        .put_token_watch_meta(
                            *token,
                            *owner,
                            &TokenWatchMeta {
                                start_block: current_block_num,
                            },
                        )
                        .context("Failed to store token watch meta")?;
                    info!(
                        "Initialized ERC20 token {:?} for owner {:?}: balance={:?}",
                        token, owner, balance
                    );
                }
            }
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

            let mut traced_tx_count: u64 = 0;
            let mut internal_credit_count: u64 = 0;
            let mut trace_failures: u64 = 0;
            let mut successful_receipts: Vec<(B256, crate::types::Receipt)> = Vec::new();

            // Process each transaction
            for tx in &block.transactions {
                // Check if transaction affects any watched address at the top level.
                // Note: internal transfers from contracts to watched EOAs are handled
                // separately via tracing and do not rely on this filter.
                let sender_watched = watchlist_set.contains(&tx.from);
                let _receiver_watched = tx.to.map_or(false, |to| watchlist_set.contains(&to));

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

                // Always fetch receipt (needed for fee calculation, status, and
                // to decide whether we should consider internal transfers).
                let receipt = self
                    .rpc
                    .get_transaction_receipt(tx.hash)
                    .await
                    .with_context(|| {
                        format!("Failed to fetch receipt for tx {:?}", tx.hash)
                    })?;

                if receipt.is_success() {
                    successful_receipts.push((tx.hash, receipt.clone()));
                }

                // 1) Internal transfers via tracing (contract → watched EOA).
                if receipt.is_success() {
                    traced_tx_count += 1;

                    match self
                        .rpc
                        .debug_trace_transaction_calltracer(tx.hash, "10s")
                        .await
                    {
                        Ok(trace) => {
                            // Preload contract flags for all senders in this trace so that
                            // the `sender_is_contract` predicate can be pure and fast.
                            let senders = collect_senders(&trace);
                            self.preload_contract_flags(&senders, block_num)
                                .await
                                .with_context(|| {
                                    format!(
                                        "Failed to preload contract flags for trace of tx {:?}",
                                        tx.hash
                                    )
                                })?;

                            // Build a simple map from address -> is_contract from the cache.
                            let mut contract_flags: HashMap<Address, bool> = HashMap::new();
                            for addr in &senders {
                                if let Some(is_contract) = self.cache.is_contract(*addr) {
                                    contract_flags.insert(*addr, is_contract);
                                }
                            }

                            let internal_transfers = collect_internal_transfers(
                                &trace,
                                true,
                                &watchlist_set,
                                |from| *contract_flags.get(&from).unwrap_or(&false),
                            );

                            for t in internal_transfers {
                                apply_internal_credit(
                                    &self.store,
                                    t.to,
                                    t.value,
                                    block_num,
                                    &mut self.block_deltas,
                                )
                                .with_context(|| {
                                    format!(
                                        "Failed to apply internal credit for tx {:?}",
                                        tx.hash
                                    )
                                })?;
                                internal_credit_count += 1;
                            }
                        }
                        Err(e) => {
                            trace_failures += 1;
                            tracing::warn!(
                                "debug_traceTransaction failed for tx {:?} in block {}: {:?}",
                                tx.hash,
                                block_num,
                                e
                            );
                            // Guardrail: do not crash the block processor; simply
                            // skip internal credits for this transaction.
                        }
                    }
                }

                // 2) Top-level EOA→EOA transfers and watched sender fee/nonce updates.
                if should_process {
                    // For EOA→EOA transfers, verify receiver is actually an EOA
                    if is_eoa_to_eoa_transfer(tx) {
                        if let Some(receiver) = tx.to {
                            let is_eoa = check_receiver_is_eoa(
                                &self.rpc,
                                &mut self.cache,
                                receiver,
                                block_num,
                            )
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

                    apply_transaction(
                        &self.store,
                        &self.rpc,
                        &mut self.cache,
                        tx,
                        &receipt,
                        &block,
                        &watchlist_set,
                        &mut self.block_deltas,
                    )
                    .await
                    .with_context(|| format!("Failed to apply transaction {:?}", tx.hash))?;
                }
            }

            // Run ERC20 tracker on successful receipts
            if !self.token_watchlist.is_empty() && !successful_receipts.is_empty() {
                let watched_tokens: HashSet<Address> =
                    self.token_watchlist.iter().copied().collect();
                let receipt_refs: Vec<(B256, &crate::types::Receipt)> = successful_receipts
                    .iter()
                    .map(|(h, r)| (*h, r))
                    .collect();
                let ctx = TrackerContext {
                    store: &self.store as &dyn StateStore,
                    rpc: &self.rpc,
                    watched_eoas: &watchlist_set,
                    watched_tokens: &watched_tokens,
                    block_number: block_num,
                };
                self.erc20_tracker
                    .process_block(&ctx, &receipt_refs)
                    .with_context(|| format!("ERC20 tracker failed for block {}", block_num))?;
            }

            // After processing all transactions in the block, persist deltas and snapshots
            // Only store entries for addresses that had changes
            for (addr, delta) in &self.block_deltas {
                // Only store if there were actual changes
                if !delta.has_changes() {
                    continue;
                }

                // Store the delta
                self.store
                    .put_delta(*addr, block_num, delta)
                    .with_context(|| {
                        format!("Failed to store delta for {:?} at block {}", addr, block_num)
                    })?;

                // Get the current balance after all transactions in this block
                let account = self
                    .store
                    .get_account(*addr)
                    .context("Failed to get account for snapshot")?
                    .ok_or_else(|| {
                        anyhow::anyhow!("Account not found for snapshot: {:?}", addr)
                    })?;

                // Store the snapshot (balance after this block)
                self.store
                    .put_snapshot(*addr, block_num, account.balance)
                    .with_context(|| {
                        format!("Failed to store snapshot for {:?} at block {}", addr, block_num)
                    })?;
            }

            // Update head after processing block
            self.store
                .set_head(block_num)
                .context("Failed to update head block")?;

            info!(
                "Completed block {} ({} addresses changed, traced_tx_count={}, internal_credits={}, trace_failures={})",
                block_num,
                self.block_deltas.len(),
                traced_tx_count,
                internal_credit_count,
                trace_failures
            );
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
