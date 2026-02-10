//! CLI implementation for statectl
//!
//! Provides a developer-friendly command-line interface for interacting
//! with the state store. All commands output pretty JSON.

use crate::records::{AccountRecord, HeaderRecord};
use crate::{RocksStateStore, StateStore};
use alloy_primitives::{Address, B256, U256};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::path::PathBuf;

/// State store CLI tool
#[derive(Parser)]
#[command(name = "statectl")]
#[command(about = "Ethereum state store CLI tool")]
pub struct Cli {
    /// Path to the RocksDB database directory
    #[arg(short, long, default_value = "./state_db")]
    db_path: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Set the head block number
    SetHead {
        /// Block number
        block: u64,
    },
    /// Get the head block number
    GetHead,
    /// Store an account record
    PutAccount {
        /// Ethereum address (hex, with or without 0x prefix)
        address: String,
        /// Nonce (u64)
        nonce: u64,
        /// Balance in hex (with or without 0x prefix)
        balance_hex: String,
        /// Code hash in hex (64 hex chars, with or without 0x prefix)
        code_hash: String,
    },
    /// Get an account record
    GetAccount {
        /// Ethereum address (hex, with or without 0x prefix)
        address: String,
    },
    /// Store contract bytecode
    PutCode {
        /// Code hash in hex (64 hex chars, with or without 0x prefix)
        code_hash: String,
        /// Bytecode in hex (with or without 0x prefix)
        hex_bytecode: String,
    },
    /// Get contract bytecode
    GetCode {
        /// Code hash in hex (64 hex chars, with or without 0x prefix)
        code_hash: String,
    },
    /// Store a storage slot value
    PutStorage {
        /// Ethereum address (hex, with or without 0x prefix)
        address: String,
        /// Storage slot in hex (64 hex chars, with or without 0x prefix)
        slot: String,
        /// Value in hex (with or without 0x prefix)
        value_hex: String,
    },
    /// Get a storage slot value
    GetStorage {
        /// Ethereum address (hex, with or without 0x prefix)
        address: String,
        /// Storage slot in hex (64 hex chars, with or without 0x prefix)
        slot: String,
    },
    /// Store a block header
    PutHeader {
        /// Block number
        number: u64,
        /// Timestamp (Unix epoch seconds)
        timestamp: u64,
        /// Base fee in hex (with or without 0x prefix)
        basefee_hex: String,
        /// Coinbase address (hex, with or without 0x prefix)
        coinbase: String,
        /// Previous RANDAO in hex (64 hex chars, with or without 0x prefix)
        prevrandao: String,
        /// Gas limit
        gas_limit: u64,
        /// Chain ID
        chain_id: u64,
    },
    /// Get a block header
    GetHeader {
        /// Block number
        number: u64,
    },
    /// Store a block hash
    PutBlockHash {
        /// Block number
        number: u64,
        /// Block hash in hex (64 hex chars, with or without 0x prefix)
        hash: String,
    },
    /// Get a block hash
    GetBlockHash {
        /// Block number
        number: u64,
    },
    /// Get block deltas for an address in a range
    Deltas {
        /// Ethereum address (hex, with or without 0x prefix)
        address: String,
        /// Start block number (inclusive)
        start: u64,
        /// End block number (inclusive)
        end: u64,
        /// Include all blocks in range, even with zero deltas
        #[arg(long)]
        dense: bool,
    },
    /// Get balances for an address in a range (fill-forward)
    Balances {
        /// Ethereum address (hex, with or without 0x prefix)
        address: String,
        /// Start block number (inclusive)
        start: u64,
        /// End block number (inclusive)
        end: u64,
    },
    /// Get ERC20 token balances for (token, owner) in a range
    Erc20Balances {
        /// ERC20 token contract address
        token: String,
        /// Owner address
        owner: String,
        /// Start block number (inclusive)
        start: u64,
        /// End block number (inclusive)
        end: u64,
    },
    /// Get ERC20 token deltas for (token, owner) in a range
    Erc20Deltas {
        /// ERC20 token contract address
        token: String,
        /// Owner address
        owner: String,
        /// Start block number (inclusive)
        start: u64,
        /// End block number (inclusive)
        end: u64,
        /// Include all blocks in range, even with zero deltas
        #[arg(long)]
        dense: bool,
    },
}

/// Pad an odd-length hex string with a leading zero.
fn pad_hex_string(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }
    if s.len() % 2 == 1 {
        format!("0{}", s)
    } else {
        s.to_string()
    }
}

/// Parse a hex string into a 20-byte address.
fn parse_address(s: &str) -> Result<Address> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let s = pad_hex_string(s);
    let bytes = hex::decode(&s)
        .with_context(|| format!("Invalid hex address: {}", s))?;
    if bytes.len() != 20 {
        anyhow::bail!("Address must be 20 bytes (40 hex chars), got {} bytes", bytes.len());
    }
    Ok(Address::from_slice(&bytes))
}

/// Parse a hex string into a 32-byte hash (B256).
fn parse_hash(s: &str) -> Result<B256> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let s = pad_hex_string(s);
    let bytes = hex::decode(&s)
        .with_context(|| format!("Invalid hex hash: {}", s))?;
    if bytes.len() != 32 {
        anyhow::bail!("Hash must be 32 bytes (64 hex chars), got {} bytes", bytes.len());
    }
    Ok(B256::from_slice(&bytes))
}

/// Parse a hex string into a U256 value.
fn parse_u256(s: &str) -> Result<U256> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.is_empty() {
        return Ok(U256::ZERO);
    }
    let s = pad_hex_string(s);
    let bytes = hex::decode(&s)
        .with_context(|| format!("Invalid hex U256: {}", s))?;
    if bytes.len() > 32 {
        anyhow::bail!("U256 value too large (max 32 bytes), got {} bytes", bytes.len());
    }
    Ok(U256::from_be_slice(&bytes))
}

/// Run the CLI command and print JSON output.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let store = RocksStateStore::open(&cli.db_path)
        .with_context(|| format!("Failed to open database at {:?}", cli.db_path))?;

    let result = match cli.command {
        Commands::SetHead { block } => {
            store.set_head(block)?;
            json!({ "status": "ok", "head_block": block })
        }
        Commands::GetHead => {
            match store.get_head()? {
                Some(block) => json!({ "head_block": block }),
                None => json!({ "head_block": null }),
            }
        }
        Commands::PutAccount {
            address,
            nonce,
            balance_hex,
            code_hash,
        } => {
            let addr = parse_address(&address)?;
            let balance = parse_u256(&balance_hex)?;
            let code_hash_val = parse_hash(&code_hash)?;
            let account = AccountRecord {
                nonce,
                balance,
                code_hash: code_hash_val,
            };
            store.put_account(addr, &account)?;
            json!({
                "status": "ok",
                "address": format!("0x{:x}", addr),
                "account": {
                    "nonce": nonce,
                    "balance": format!("0x{:x}", balance),
                    "code_hash": format!("0x{:x}", code_hash_val),
                }
            })
        }
        Commands::GetAccount { address } => {
            let addr = parse_address(&address)?;
            match store.get_account(addr)? {
                Some(acc) => json!({
                    "address": format!("0x{:x}", addr),
                    "account": {
                        "nonce": acc.nonce,
                        "balance": format!("0x{:x}", acc.balance),
                        "code_hash": format!("0x{:x}", acc.code_hash),
                    }
                }),
                None => json!({
                    "address": format!("0x{:x}", addr),
                    "account": null
                }),
            }
        }
        Commands::PutCode { code_hash, hex_bytecode } => {
            let code_hash_val = parse_hash(&code_hash)?;
            let code_hex = hex_bytecode.strip_prefix("0x").unwrap_or(&hex_bytecode);
            let code_hex = pad_hex_string(code_hex);
            let code = hex::decode(&code_hex)
                .context("Invalid hex bytecode")?;
            store.put_code(code_hash_val, &code)?;
            json!({
                "status": "ok",
                "code_hash": format!("0x{:x}", code_hash_val),
                "code_length": code.len(),
            })
        }
        Commands::GetCode { code_hash } => {
            let code_hash_val = parse_hash(&code_hash)?;
            match store.get_code(code_hash_val)? {
                Some(code) => json!({
                    "code_hash": format!("0x{:x}", code_hash_val),
                    "code": format!("0x{}", hex::encode(&code)),
                    "code_length": code.len(),
                }),
                None => json!({
                    "code_hash": format!("0x{:x}", code_hash_val),
                    "code": null
                }),
            }
        }
        Commands::PutStorage { address, slot, value_hex } => {
            let addr = parse_address(&address)?;
            let slot_val = parse_hash(&slot)?;
            let value = parse_u256(&value_hex)?;
            store.put_storage(addr, slot_val, value)?;
            json!({
                "status": "ok",
                "address": format!("0x{:x}", addr),
                "slot": format!("0x{:x}", slot_val),
                "value": format!("0x{:x}", value),
            })
        }
        Commands::GetStorage { address, slot } => {
            let addr = parse_address(&address)?;
            let slot_val = parse_hash(&slot)?;
            let value = store.get_storage(addr, slot_val)?;
            json!({
                "address": format!("0x{:x}", addr),
                "slot": format!("0x{:x}", slot_val),
                "value": format!("0x{:x}", value),
            })
        }
        Commands::PutHeader {
            number,
            timestamp,
            basefee_hex,
            coinbase,
            prevrandao,
            gas_limit,
            chain_id,
        } => {
            let basefee = parse_u256(&basefee_hex)?;
            let coinbase_addr = parse_address(&coinbase)?;
            let prevrandao_val = parse_hash(&prevrandao)?;
            let header = HeaderRecord {
                number,
                timestamp,
                basefee,
                coinbase: coinbase_addr,
                prevrandao: prevrandao_val,
                gas_limit,
                chain_id,
            };
            store.put_header(number, &header)?;
            json!({
                "status": "ok",
                "block": number,
                "header": {
                    "number": number,
                    "timestamp": timestamp,
                    "basefee": format!("0x{:x}", basefee),
                    "coinbase": format!("0x{:x}", coinbase_addr),
                    "prevrandao": format!("0x{:x}", prevrandao_val),
                    "gas_limit": gas_limit,
                    "chain_id": chain_id,
                }
            })
        }
        Commands::GetHeader { number } => {
            match store.get_header(number)? {
                Some(header) => json!({
                    "block": number,
                    "header": {
                        "number": header.number,
                        "timestamp": header.timestamp,
                        "basefee": format!("0x{:x}", header.basefee),
                        "coinbase": format!("0x{:x}", header.coinbase),
                        "prevrandao": format!("0x{:x}", header.prevrandao),
                        "gas_limit": header.gas_limit,
                        "chain_id": header.chain_id,
                    }
                }),
                None => json!({
                    "block": number,
                    "header": null
                }),
            }
        }
        Commands::PutBlockHash { number, hash } => {
            let hash_val = parse_hash(&hash)?;
            store.put_block_hash(number, hash_val)?;
            json!({
                "status": "ok",
                "block": number,
                "hash": format!("0x{:x}", hash_val),
            })
        }
        Commands::GetBlockHash { number } => {
            match store.get_block_hash(number)? {
                Some(hash) => json!({
                    "block": number,
                    "hash": format!("0x{:x}", hash),
                }),
                None => json!({
                    "block": number,
                    "hash": null
                }),
            }
        }
        Commands::Deltas {
            address,
            start,
            end,
            dense,
        } => {
            let addr = parse_address(&address)?;
            let query_result = store
                .get_deltas_in_range_with_metadata(addr, start, end)
                .context("Failed to get deltas")?;

            if dense {
                // Dense output: include all blocks in effective range, even with zero deltas
                let mut result = Vec::new();
                let mut delta_map: std::collections::HashMap<u64, _> =
                    query_result.data.into_iter().collect();
                for block in query_result.effective_start..=query_result.effective_end {
                    if let Some(delta) = delta_map.remove(&block) {
                        result.push(json!({
                            "block": block,
                            "delta_plus": format!("0x{:x}", delta.delta_plus),
                            "delta_minus": format!("0x{:x}", delta.delta_minus),
                            "received_value": format!("0x{:x}", delta.received_value),
                            "sent_value": format!("0x{:x}", delta.sent_value),
                            "fee_paid": format!("0x{:x}", delta.fee_paid),
                            "failed_fee": format!("0x{:x}", delta.failed_fee),
                            "nonce_delta": delta.nonce_delta,
                            "tx_count": delta.tx_count,
                        }));
                    } else {
                        result.push(json!({
                            "block": block,
                            "delta_plus": "0x0",
                            "delta_minus": "0x0",
                            "received_value": "0x0",
                            "sent_value": "0x0",
                            "fee_paid": "0x0",
                            "failed_fee": "0x0",
                            "nonce_delta": 0,
                            "tx_count": 0,
                        }));
                    }
                }
                json!({
                    "address": format!("0x{:x}", addr),
                    "requestedStart": query_result.requested_start,
                    "requestedEnd": query_result.requested_end,
                    "effectiveStart": query_result.effective_start,
                    "effectiveEnd": query_result.effective_end,
                    "watchStartBlock": query_result.watch_start_block,
                    "headBlock": query_result.head_block,
                    "message": query_result.message,
                    "deltas": result
                })
            } else {
                // Sparse output: only blocks with deltas
                let result: Vec<_> = query_result
                    .data
                    .into_iter()
                    .map(|(block, delta)| {
                        json!({
                            "block": block,
                            "delta_plus": format!("0x{:x}", delta.delta_plus),
                            "delta_minus": format!("0x{:x}", delta.delta_minus),
                            "received_value": format!("0x{:x}", delta.received_value),
                            "sent_value": format!("0x{:x}", delta.sent_value),
                            "fee_paid": format!("0x{:x}", delta.fee_paid),
                            "failed_fee": format!("0x{:x}", delta.failed_fee),
                            "nonce_delta": delta.nonce_delta,
                            "tx_count": delta.tx_count,
                        })
                    })
                    .collect();
                json!({
                    "address": format!("0x{:x}", addr),
                    "requestedStart": query_result.requested_start,
                    "requestedEnd": query_result.requested_end,
                    "effectiveStart": query_result.effective_start,
                    "effectiveEnd": query_result.effective_end,
                    "watchStartBlock": query_result.watch_start_block,
                    "headBlock": query_result.head_block,
                    "message": query_result.message,
                    "deltas": result
                })
            }
        }
        Commands::Balances {
            address,
            start,
            end,
        } => {
            let addr = parse_address(&address)?;
            let query_result = store
                .get_balances_in_range_with_metadata(addr, start, end)
                .context("Failed to get balances")?;

            let result: Vec<_> = query_result
                .data
                .into_iter()
                .map(|(block, balance)| {
                    json!({
                        "block": block,
                        "balance": format!("0x{:x}", balance),
                    })
                })
                .collect();

            json!({
                "address": format!("0x{:x}", addr),
                "requestedStart": query_result.requested_start,
                "requestedEnd": query_result.requested_end,
                "effectiveStart": query_result.effective_start,
                "effectiveEnd": query_result.effective_end,
                "watchStartBlock": query_result.watch_start_block,
                "headBlock": query_result.head_block,
                "message": query_result.message,
                "balances": result
            })
        }
        Commands::Erc20Balances {
            token,
            owner,
            start,
            end,
        } => {
            let token_addr = parse_address(&token)?;
            let owner_addr = parse_address(&owner)?;
            let query_result = store
                .get_erc20_balances_in_range_with_metadata(
                    token_addr,
                    owner_addr,
                    start,
                    end,
                )
                .context("Failed to get ERC20 balances")?;

            let result: Vec<_> = query_result
                .data
                .into_iter()
                .map(|(block, balance)| {
                    json!({
                        "block": block,
                        "balance": format!("0x{:x}", balance),
                    })
                })
                .collect();

            json!({
                "token": format!("0x{:x}", token_addr),
                "owner": format!("0x{:x}", owner_addr),
                "requestedStart": query_result.requested_start,
                "requestedEnd": query_result.requested_end,
                "effectiveStart": query_result.effective_start,
                "effectiveEnd": query_result.effective_end,
                "watchStartBlock": query_result.watch_start_block,
                "headBlock": query_result.head_block,
                "message": query_result.message,
                "balances": result
            })
        }
        Commands::Erc20Deltas {
            token,
            owner,
            start,
            end,
            dense,
        } => {
            let token_addr = parse_address(&token)?;
            let owner_addr = parse_address(&owner)?;
            let query_result = store
                .get_erc20_deltas_in_range_with_metadata(
                    token_addr,
                    owner_addr,
                    start,
                    end,
                )
                .context("Failed to get ERC20 deltas")?;

            if dense {
                let mut result = Vec::new();
                let mut delta_map: std::collections::HashMap<u64, _> =
                    query_result.data.into_iter().collect();
                for block in query_result.effective_start..=query_result.effective_end {
                    if let Some(delta) = delta_map.remove(&block) {
                        result.push(json!({
                            "block": block,
                            "delta_plus": format!("0x{:x}", delta.delta_plus),
                            "delta_minus": format!("0x{:x}", delta.delta_minus),
                            "tx_count": delta.tx_count,
                        }));
                    } else {
                        result.push(json!({
                            "block": block,
                            "delta_plus": "0x0",
                            "delta_minus": "0x0",
                            "tx_count": 0,
                        }));
                    }
                }
                json!({
                    "token": format!("0x{:x}", token_addr),
                    "owner": format!("0x{:x}", owner_addr),
                    "requestedStart": query_result.requested_start,
                    "requestedEnd": query_result.requested_end,
                    "effectiveStart": query_result.effective_start,
                    "effectiveEnd": query_result.effective_end,
                    "watchStartBlock": query_result.watch_start_block,
                    "headBlock": query_result.head_block,
                    "message": query_result.message,
                    "deltas": result
                })
            } else {
                let result: Vec<_> = query_result
                    .data
                    .into_iter()
                    .map(|(block, delta)| {
                        json!({
                            "block": block,
                            "delta_plus": format!("0x{:x}", delta.delta_plus),
                            "delta_minus": format!("0x{:x}", delta.delta_minus),
                            "tx_count": delta.tx_count,
                        })
                    })
                    .collect();
                json!({
                    "token": format!("0x{:x}", token_addr),
                    "owner": format!("0x{:x}", owner_addr),
                    "requestedStart": query_result.requested_start,
                    "requestedEnd": query_result.requested_end,
                    "effectiveStart": query_result.effective_start,
                    "effectiveEnd": query_result.effective_end,
                    "watchStartBlock": query_result.watch_start_block,
                    "headBlock": query_result.head_block,
                    "message": query_result.message,
                    "deltas": result
                })
            }
        }
    };

    // Pretty print JSON
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
