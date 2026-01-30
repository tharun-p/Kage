//! statectl - Ethereum state store CLI tool
//!
//! A developer-friendly command-line interface for managing Ethereum state data
//! in a persistent RocksDB store.

use kage::cli;

fn main() {
    if let Err(e) = cli::run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
