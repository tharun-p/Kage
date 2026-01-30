//! Kage - Ethereum transaction simulation state store
//!
//! This library provides a persistent key-value store for Ethereum state,
//! including accounts, contract bytecode, storage slots, block headers,
//! and block hashes.

pub mod keys;
pub mod records;
pub mod store;
pub mod cli;

// Watcher modules
pub mod apply;
pub mod cache;
pub mod config;
pub mod fee;
pub mod rpc;
pub mod types;
pub mod watcher;

// Re-export the main types for convenience
pub use records::{AccountRecord, HeaderRecord};
pub use store::{RocksStateStore, StateStore};
