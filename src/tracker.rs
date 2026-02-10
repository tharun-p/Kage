//! Tracker trait and context for extensible block processing
//!
//! Provides a modular pipeline where different trackers (ETH, ERC20, etc.)
//! can process blocks and persist their own state.
//! Future-proofs the system for DeFi protocols that may need storage reading.

use crate::rpc::RpcClient;
use crate::store::StateStore;
use alloy_primitives::Address;
use anyhow::Result;
use std::collections::HashSet;

/// Shared context passed to trackers during block processing.
///
/// Contains everything a tracker needs: store, RPC, watched addresses,
/// watched tokens, and the current block number.
pub struct TrackerContext<'a> {
    /// State store for persisting deltas/snapshots
    pub store: &'a dyn StateStore,
    /// RPC client for balanceOf, storage reads, etc.
    pub rpc: &'a RpcClient,
    /// Set of watched EOA addresses
    pub watched_eoas: &'a HashSet<Address>,
    /// Set of watched ERC20 token contract addresses
    pub watched_tokens: &'a HashSet<Address>,
    /// Current block number being processed
    pub block_number: u64,
}

/// Block-processing tracker trait.
///
/// Each tracker receives block context and receipts, and may persist
/// its own deltas/snapshots. Trackers are called in sequence by the watcher.
pub trait Tracker {
    /// Human-readable name for logging.
    fn name(&self) -> &'static str;

    /// Process a block.
    ///
    /// Receives the context and a list of (tx_hash, receipt) for successful
    /// transactions only. The tracker may fetch additional data via RPC if needed.
    fn process_block(
        &self,
        _ctx: &TrackerContext<'_>,
        _receipts: &[(alloy_primitives::B256, &crate::types::Receipt)],
    ) -> Result<()> {
        // Default no-op for optional processing
        Ok(())
    }
}
