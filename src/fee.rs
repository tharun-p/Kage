//! Gas fee calculation utilities
//!
//! Handles calculation of effective gas price and total fees
//! for both legacy and EIP-1559 transactions.

use crate::types::{Block, Receipt, Transaction};
use alloy_primitives::U256;
use anyhow::{Context, Result};

/// Calculate the effective gas price for a transaction.
///
/// Priority order:
/// 1. Use `effective_gas_price` from receipt if available (post-London)
/// 2. Use `gas_price` for legacy transactions
/// 3. Calculate for EIP-1559: `min(max_fee, base_fee + max_priority_fee)`
pub fn calculate_effective_gas_price(
    tx: &Transaction,
    receipt: &Receipt,
    block: &Block,
) -> Result<U256> {
    // First, try to use the effective gas price from the receipt (most accurate)
    if let Some(egp) = receipt.effective_gas_price {
        return Ok(egp);
    }

    // For legacy transactions, use the gas price directly
    if tx.is_legacy() {
        return tx
            .gas_price
            .context("Legacy transaction missing gas_price");
    }

    // For EIP-1559 transactions, calculate: min(max_fee, base_fee + max_priority_fee)
    if tx.is_eip1559() {
        let base_fee = block
            .base_fee_per_gas
            .context("EIP-1559 transaction but block missing base_fee_per_gas")?;

        let max_fee = tx
            .max_fee_per_gas
            .context("EIP-1559 transaction missing max_fee_per_gas")?;

        let max_priority_fee = tx
            .max_priority_fee_per_gas
            .unwrap_or(U256::ZERO);

        // effective_gas_price = min(max_fee, base_fee + max_priority_fee)
        let calculated = base_fee.saturating_add(max_priority_fee);
        let effective = if calculated > max_fee {
            max_fee
        } else {
            calculated
        };

        return Ok(effective);
    }

    anyhow::bail!("Transaction type not recognized (neither legacy nor EIP-1559)");
}

/// Calculate the total fee paid for a transaction.
///
/// Fee = gas_used * effective_gas_price
pub fn calculate_fee(gas_used: U256, effective_gas_price: U256) -> U256 {
    gas_used.saturating_mul(effective_gas_price)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, b256};

    fn create_test_block(base_fee: Option<U256>) -> Block {
        Block {
            number: 12345,
            hash: b256!("0000000000000000000000000000000000000000000000000000000000000000"),
            base_fee_per_gas: base_fee,
            transactions: vec![],
        }
    }

    fn create_legacy_tx(gas_price: U256) -> Transaction {
        Transaction {
            hash: b256!("0000000000000000000000000000000000000000000000000000000000000000"),
            from: address!("0000000000000000000000000000000000000000"),
            to: Some(address!("0000000000000000000000000000000000000001")),
            value: U256::ZERO,
            gas_price: Some(gas_price),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            gas: U256::from(21000),
            input: vec![],
            nonce: 0,
        }
    }

    fn create_eip1559_tx(max_fee: U256, max_priority_fee: U256) -> Transaction {
        Transaction {
            hash: b256!("0000000000000000000000000000000000000000000000000000000000000000"),
            from: address!("0000000000000000000000000000000000000000"),
            to: Some(address!("0000000000000000000000000000000000000001")),
            value: U256::ZERO,
            gas_price: None,
            max_fee_per_gas: Some(max_fee),
            max_priority_fee_per_gas: Some(max_priority_fee),
            gas: U256::from(21000),
            input: vec![],
            nonce: 0,
        }
    }

    fn create_receipt(gas_used: U256, effective_gas_price: Option<U256>) -> Receipt {
        Receipt {
            status: 1,
            gas_used,
            effective_gas_price,
            logs: vec![],
        }
    }

    #[test]
    fn test_legacy_fee_calculation() {
        let block = create_test_block(None);
        let tx = create_legacy_tx(U256::from(20_000_000_000u64)); // 20 gwei
        let receipt = create_receipt(U256::from(21000), None);

        let effective = calculate_effective_gas_price(&tx, &receipt, &block).unwrap();
        assert_eq!(effective, U256::from(20_000_000_000u64));

        let fee = calculate_fee(receipt.gas_used, effective);
        // 21000 * 20_000_000_000 = 420_000_000_000_000
        assert_eq!(fee, U256::from(420_000_000_000_000u64)); // 21000 * 20 gwei
    }

    #[test]
    fn test_eip1559_fee_calculation() {
        let base_fee = U256::from(10_000_000_000u64); // 10 gwei
        let block = create_test_block(Some(base_fee));
        let max_fee = U256::from(30_000_000_000u64); // 30 gwei
        let max_priority_fee = U256::from(2_000_000_000u64); // 2 gwei
        let tx = create_eip1559_tx(max_fee, max_priority_fee);
        let receipt = create_receipt(U256::from(21000), None);

        let effective = calculate_effective_gas_price(&tx, &receipt, &block).unwrap();
        // effective = min(30, 10 + 2) = 12 gwei
        assert_eq!(effective, U256::from(12_000_000_000u64));

        let fee = calculate_fee(receipt.gas_used, effective);
        // 21000 * 12_000_000_000 = 252_000_000_000_000
        assert_eq!(fee, U256::from(252_000_000_000_000u64)); // 21000 * 12 gwei
    }

    #[test]
    fn test_eip1559_fee_capped_by_max_fee() {
        let base_fee = U256::from(50_000_000_000u64); // 50 gwei
        let block = create_test_block(Some(base_fee));
        let max_fee = U256::from(30_000_000_000u64); // 30 gwei (lower than base + priority)
        let max_priority_fee = U256::from(2_000_000_000u64); // 2 gwei
        let tx = create_eip1559_tx(max_fee, max_priority_fee);
        let receipt = create_receipt(U256::from(21000), None);

        let effective = calculate_effective_gas_price(&tx, &receipt, &block).unwrap();
        // effective = min(30, 50 + 2) = 30 gwei (capped by max_fee)
        assert_eq!(effective, max_fee);
    }

    #[test]
    fn test_receipt_effective_gas_price_takes_priority() {
        let block = create_test_block(Some(U256::from(10_000_000_000u64)));
        let tx = create_eip1559_tx(
            U256::from(30_000_000_000u64),
            U256::from(2_000_000_000u64),
        );
        // Receipt has effective_gas_price, should use that instead of calculating
        let receipt = create_receipt(U256::from(21000), Some(U256::from(15_000_000_000u64)));

        let effective = calculate_effective_gas_price(&tx, &receipt, &block).unwrap();
        assert_eq!(effective, U256::from(15_000_000_000u64));
    }
}
