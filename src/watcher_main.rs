//! Ethereum address watcher binary
//!
//! Monitors finalized blocks and updates local state for watched EOA addresses.
//! Handles EOA→EOA ETH transfers with correct gas/fee accounting.

use kage::rpc::RpcClient;
use kage::store::RocksStateStore;
use kage::watcher::Watcher;
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber;

/// Ethereum address watcher
#[derive(Parser)]
#[command(name = "watcher")]
#[command(about = "Monitor Ethereum blocks and update state for watched addresses")]
struct Args {
    /// RPC endpoint URL (e.g., https://eth.llamarpc.com)
    #[arg(short, long, default_value = "http://127.0.0.1:8545")]
    rpc_url: String,

    /// Path to watchlist file (one address per line)
    #[arg(short, long, default_value = "watchlist.txt")]
    watchlist: PathBuf,

    /// Path to token watchlist file (one ERC20 contract address per line, optional)
    #[arg(short, long)]
    tokens: Option<PathBuf>,

    /// Path to RocksDB database directory
    #[arg(short, long, default_value = "./state_db")]
    db_path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let args = Args::parse();

    info!("Starting Ethereum address watcher");
    info!("RPC URL: {}", args.rpc_url);
    info!("Watchlist: {:?}", args.watchlist);
    info!("Database: {:?}", args.db_path);

    // Create RPC client
    let rpc = RpcClient::new(args.rpc_url);

    // Open state store
    let store = RocksStateStore::open(&args.db_path)
        .with_context(|| format!("Failed to open database at {:?}", args.db_path))?;

    // Create watcher
    let mut watcher = Watcher::new(store, rpc);

    // Initialize (load watchlist, fetch initial state, optionally ERC20 tokens)
    watcher
        .initialize(&args.watchlist, args.tokens.as_deref())
        .await
        .context("Failed to initialize watcher")?;

    // Handle Ctrl+C gracefully
    let mut watcher = watcher;
    tokio::select! {
        result = watcher.run() => {
            result.context("Watcher error")?;
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down gracefully...");
        }
    }

    info!("Watcher stopped");
    Ok(())
}
