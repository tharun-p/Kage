//! Transaction application logic
//!
//! Handles filtering EOA→EOA transfers and applying them to the state store.
//! Updates balances and nonces for watched addresses.

use crate::cache::ContractCache;
use crate::fee::{calculate_effective_gas_price, calculate_fee};
use crate::rpc::RpcClient;
use crate::store::StateStore;
use crate::types::{Block, Receipt, Transaction};
use alloy_primitives::{Address, U256};
use anyhow::{Context, Result};
use std::collections::HashSet;
use tracing::{debug, info};

/// Check if a transaction is a simple EOA→EOA ETH transfer.
///
/// Criteria:
/// - `to` is present (not contract creation)
/// - `value > 0` (has value transfer)
/// - `input` is empty (no contract call data)
pub fn is_eoa_to_eoa_transfer(tx: &Transaction) -> bool {
    tx.to.is_some() && tx.value > U256::ZERO && tx.input.is_empty()
}

/// Check if the receiver address is an EOA (not a contract).
///
/// Uses the cache first, then RPC if needed.
/// Updates the cache with the result.
pub async fn check_receiver_is_eoa(
    rpc: &RpcClient,
    cache: &mut ContractCache,
    addr: Address,
    block: u64,
) -> Result<bool> {
    // Check cache first
    if let Some(is_contract) = cache.is_contract(addr) {
        return Ok(!is_contract); // Return true if EOA (not contract)
    }

    // Not in cache, check via RPC
    let code = rpc
        .get_code(addr, &format!("0x{:x}", block))
        .await
        .context("Failed to get code for receiver")?;

    let is_eoa = code.is_empty();
    cache.mark_contract(addr, !is_eoa); // Cache: true = contract, false = EOA

    Ok(is_eoa)
}

/// Apply a transaction to the state store.
///
/// Updates balances and nonces for watched addresses.
/// Only processes transactions that affect addresses in the watchlist.
///
/// For watched senders: Always processes to deduct fees and update nonce.
/// For watched receivers: Only processes EOA→EOA transfers (value > 0, no input data).
pub async fn apply_transaction(
    store: &dyn StateStore,
    _rpc: &RpcClient,
    _cache: &mut ContractCache,
    tx: &Transaction,
    receipt: &Receipt,
    block: &Block,
    watchlist: &HashSet<Address>,
) -> Result<()> {
    let sender = tx.from;
    let receiver = tx.to;
    let value = tx.value;
    let tx_succeeded = receipt.is_success();
    let is_simple_transfer = is_eoa_to_eoa_transfer(tx);

    // Calculate effective gas price and fee
    let effective_gas_price = calculate_effective_gas_price(tx, receipt, block)
        .context("Failed to calculate effective gas price")?;
    let fee = calculate_fee(receipt.gas_used, effective_gas_price);

    // Update sender if in watchlist (ALWAYS process sender transactions)
    if watchlist.contains(&sender) {
        // Get current account state
        let mut account = store
            .get_account(sender)
            .context("Failed to get sender account")?
            .unwrap_or_else(|| {
                // Account doesn't exist, create with zero balance/nonce
                // (shouldn't happen if initialized, but handle gracefully)
                crate::records::AccountRecord {
                    nonce: 0,
                    balance: U256::ZERO,
                    code_hash: alloy_primitives::B256::ZERO,
                }
            });

        let balance_before = account.balance;
        let nonce_before = account.nonce;

        // Update balance based on transaction outcome
        if tx_succeeded {
            // Success: sender pays value + fee
            account.balance = account
                .balance
                .saturating_sub(value)
                .saturating_sub(fee);
        } else {
            // Failure: sender only pays fee
            account.balance = account.balance.saturating_sub(fee);
        }

        // Update nonce (always increments, even on failure)
        account.nonce += 1;

        // Save updated account
        store
            .put_account(sender, &account)
            .context("Failed to save sender account")?;

        info!(
            "TX {:?}: sender {:?} balance {} -> {} (value={}, fee={}, gas_used={}, egp={}), nonce {} -> {}",
            tx.hash, sender, balance_before, account.balance, value, fee, receipt.gas_used, effective_gas_price, nonce_before, account.nonce
        );
    }

    // Update receiver if in watchlist, transaction succeeded, and it's a simple transfer
    // (We only credit receivers for EOA→EOA transfers, not contract interactions)
    if let Some(recv) = receiver {
        if watchlist.contains(&recv) && tx_succeeded && is_simple_transfer && value > U256::ZERO {
            // Get current account state
            let mut account = store
                .get_account(recv)
                .context("Failed to get receiver account")?
                .unwrap_or_else(|| {
                    // Account doesn't exist, create with zero balance/nonce
                    crate::records::AccountRecord {
                        nonce: 0,
                        balance: U256::ZERO,
                        code_hash: alloy_primitives::B256::ZERO,
                    }
                });

            let balance_before = account.balance;
            // Receiver gets the value
            account.balance = account.balance.saturating_add(value);

            // Save updated account
            store
                .put_account(recv, &account)
                .context("Failed to save receiver account")?;

            info!(
                "TX {:?}: receiver {:?} balance {} -> {} (value={})",
                tx.hash, recv, balance_before, account.balance, value
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::AccountRecord;
    use crate::store::RocksStateStore;
    use alloy_primitives::{address, b256};
    use tempfile::TempDir;

    fn create_test_tx(
        from: Address,
        to: Option<Address>,
        value: U256,
        input: Vec<u8>,
    ) -> Transaction {
        Transaction {
            hash: b256!("0000000000000000000000000000000000000000000000000000000000000000"),
            from,
            to,
            value,
            gas_price: Some(U256::from(20_000_000_000u64)),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            gas: U256::from(21000),
            input,
            nonce: 0,
        }
    }

    fn create_test_receipt(status: u64, gas_used: U256) -> Receipt {
        Receipt {
            status,
            gas_used,
            effective_gas_price: None,
        }
    }

    fn create_test_block() -> Block {
        Block {
            number: 12345,
            hash: b256!("0000000000000000000000000000000000000000000000000000000000000000"),
            base_fee_per_gas: None,
            transactions: vec![],
        }
    }

    #[test]
    fn test_is_eoa_to_eoa_transfer() {
        let from = address!("0000000000000000000000000000000000000001");
        let to = address!("0000000000000000000000000000000000000002");

        // Valid transfer
        let tx1 = create_test_tx(from, Some(to), U256::from(1000), vec![]);
        assert!(is_eoa_to_eoa_transfer(&tx1));

        // No value
        let tx2 = create_test_tx(from, Some(to), U256::ZERO, vec![]);
        assert!(!is_eoa_to_eoa_transfer(&tx2));

        // Has input data
        let tx3 = create_test_tx(from, Some(to), U256::from(1000), vec![0x60, 0x00]);
        assert!(!is_eoa_to_eoa_transfer(&tx3));

        // Contract creation
        let tx4 = create_test_tx(from, None, U256::from(1000), vec![]);
        assert!(!is_eoa_to_eoa_transfer(&tx4));
    }

    // Note: Integration tests for apply_transaction would require a mock RPC client
    // For now, we test the filter logic which is the most critical part
}
