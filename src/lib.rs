//! Kage - Ethereum transaction simulation state store
//!
//! This library provides a persistent key-value store for Ethereum state,
//! including accounts, contract bytecode, storage slots, block headers,
//! and block hashes.

pub mod keys;
pub mod records;
pub mod store;
pub mod cli;
pub mod trace;
pub mod tracker;
pub mod tracker_erc20;

// Watcher modules
pub mod apply;
pub mod cache;
pub mod config;
pub mod fee;
pub mod rpc;
pub mod types;
pub mod watcher;

// Re-export the main types for convenience
pub use records::{
    AccountRecord, BalanceSnapshot, BlockDelta, Erc20Delta, Erc20Snapshot, HeaderRecord,
    TokenWatchMeta, WatchMeta,
};
pub use store::{QueryResult, RocksStateStore, StateStore};
