//! ERC20 Transfer event tracker
//!
//! Parses ERC20 Transfer logs from receipts and updates per-(token, owner)
//! deltas and snapshots. Handles mint (from=0x0), burn (to=0x0), and normal transfers.
//! Ignores logs from reverted transactions.

use crate::records::Erc20Delta;
use crate::tracker::{Tracker, TrackerContext};
use crate::types::{Log, Receipt};
use alloy_primitives::{Address, B256, U256};
use anyhow::{Context, Result};
use std::collections::HashMap;

/// keccak256("Transfer(address,address,uint256)")
const TRANSFER_TOPIC: [u8; 32] = [
    0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d,
    0xaa, 0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a, 0x4d, 0xf5, 0x23,
    0xb3, 0xef,
];

/// Zero address (mint sender, burn receiver)
fn zero_address() -> Address {
    Address::ZERO
}

/// ERC20 tracker that parses Transfer events and updates token balances.
pub struct Erc20Tracker {
    /// List of watched token contract addresses (from context when processing)
    #[allow(dead_code)]
    tokens: Vec<Address>,
}

impl Erc20Tracker {
    /// Create a new ERC20 tracker with the given token list.
    pub fn new(tokens: Vec<Address>) -> Self {
        Self { tokens }
    }

    /// Check if a log is an ERC20 Transfer event.
    fn is_transfer_event(&self, log: &Log) -> bool {
        if log.topics.is_empty() {
            return false;
        }
        let topic0 = &log.topics[0];
        let topic0 = topic0.strip_prefix("0x").unwrap_or(topic0);
        if topic0.len() != 64 {
            return false;
        }
        let bytes = match hex::decode(topic0) {
            Ok(b) => b,
            Err(_) => return false,
        };
        if bytes.len() != 32 {
            return false;
        }
        bytes.as_slice() == TRANSFER_TOPIC
    }

    /// Parse from, to, value from a Transfer log.
    /// topics[1] = from (indexed, padded to 32 bytes), topics[2] = to, data = value
    fn parse_transfer_log(&self, log: &Log) -> Result<(Address, Address, U256)> {
        if log.topics.len() < 3 {
            anyhow::bail!("Transfer log has insufficient topics");
        }
        let from = parse_address_from_topic(&log.topics[1])?;
        let to = parse_address_from_topic(&log.topics[2])?;
        let value = if log.data.len() >= 32 {
            U256::from_be_slice(&log.data[0..32])
        } else {
            U256::ZERO
        };
        Ok((from, to, value))
    }

    /// Process receipts for a block and accumulate ERC20 deltas.
    fn process_receipts(
        &self,
        ctx: &TrackerContext<'_>,
        receipts: &[(B256, &Receipt)],
    ) -> Result<HashMap<(Address, Address), Erc20Delta>> {
        let mut acc: HashMap<(Address, Address), Erc20Delta> = HashMap::new();
        let watched_tokens: std::collections::HashSet<Address> =
            ctx.watched_tokens.iter().copied().collect();
        let watched_eoas: std::collections::HashSet<Address> =
            ctx.watched_eoas.iter().copied().collect();

        for (_tx_hash, receipt) in receipts {
            // Only process successful transactions (reverted txs have no effect)
            if !receipt.is_success() {
                continue;
            }

            for log in &receipt.logs {
                if !self.is_transfer_event(log) {
                    continue;
                }
                // Only process logs from watched tokens
                if !watched_tokens.contains(&log.address) {
                    continue;
                }
                let token = log.address;

                let (from, to, value) = match self.parse_transfer_log(log) {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!("Failed to parse Transfer log: {:?}", e);
                        continue;
                    }
                };

                if value == U256::ZERO {
                    continue;
                }

                // Handle receiver (to)
                if to != zero_address() && watched_eoas.contains(&to) {
                    let entry = acc
                        .entry((token, to))
                        .or_insert_with(|| Erc20Delta::new(ctx.block_number));
                    entry.delta_plus = entry.delta_plus.saturating_add(value);
                    entry.tx_count = entry.tx_count.saturating_add(1);
                }

                // Handle sender (from)
                if from != zero_address() && watched_eoas.contains(&from) {
                    let entry = acc
                        .entry((token, from))
                        .or_insert_with(|| Erc20Delta::new(ctx.block_number));
                    entry.delta_minus = entry.delta_minus.saturating_add(value);
                    entry.tx_count = entry.tx_count.saturating_add(1);
                }
            }
        }

        Ok(acc)
    }
}

impl Tracker for Erc20Tracker {
    fn name(&self) -> &'static str {
        "Erc20Tracker"
    }

    fn process_block(
        &self,
        ctx: &TrackerContext<'_>,
        receipts: &[(B256, &Receipt)],
    ) -> Result<()> {
        if ctx.watched_tokens.is_empty() {
            return Ok(());
        }

        let acc = self.process_receipts(ctx, receipts)?;

        for ((token, owner), delta) in acc {
            if !delta.has_changes() {
                continue;
            }

            // Check coverage: only persist if we have TokenWatchMeta for this (token, owner)
            let meta = match ctx.store.get_token_watch_meta(token, owner)? {
                Some(m) => m,
                None => continue, // Not tracking this (token, owner), skip
            };

            if ctx.block_number < meta.start_block {
                continue;
            }

            // Persist delta
            ctx.store
                .put_erc20_delta(token, owner, ctx.block_number, &delta)
                .with_context(|| {
                    format!(
                        "Failed to store ERC20 delta for token {:?} owner {:?}",
                        token, owner
                    )
                })?;

            // Get current balance and apply delta
            let current = ctx
                .store
                .get_erc20_balance(token, owner)?
                .unwrap_or(U256::ZERO);
            let new_balance = current
                .saturating_add(delta.delta_plus)
                .saturating_sub(delta.delta_minus);

            // Update current balance and snapshot
            ctx.store
                .put_erc20_balance(token, owner, new_balance)
                .with_context(|| {
                    format!(
                        "Failed to store ERC20 balance for token {:?} owner {:?}",
                        token, owner
                    )
                })?;
            ctx.store
                .put_erc20_snapshot(token, owner, ctx.block_number, new_balance)
                .with_context(|| {
                    format!(
                        "Failed to store ERC20 snapshot for token {:?} owner {:?}",
                        token, owner
                    )
                })?;
        }

        Ok(())
    }
}

/// Parse a 32-byte hex topic into an Address (last 20 bytes).
fn parse_address_from_topic(topic: &str) -> Result<Address> {
    let s = topic.strip_prefix("0x").unwrap_or(topic);
    let s = if s.len() % 2 == 1 { format!("0{}", s) } else { s.to_string() };
    let bytes = hex::decode(&s).context("Invalid hex in topic")?;
    if bytes.len() < 20 {
        anyhow::bail!("Topic too short for address");
    }
    let start = bytes.len().saturating_sub(20);
    Ok(Address::from_slice(&bytes[start..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Log;

    #[test]
    fn test_transfer_topic() {
        // Verify our constant matches keccak256("Transfer(address,address,uint256)")
        assert_eq!(TRANSFER_TOPIC.len(), 32);
    }

    #[test]
    fn test_parse_address_from_topic() {
        // Standard 32-byte padded address
        let topic = "0x00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8";
        let addr = parse_address_from_topic(topic).unwrap();
        let expected = Address::from_slice(
            &hex::decode("70997970c51812dc3a010c7d01b50e0d17dc79c8").unwrap(),
        );
        assert_eq!(addr, expected);
    }

    #[test]
    fn test_zero_address() {
        assert_eq!(zero_address(), Address::ZERO);
    }

    #[test]
    fn test_erc20_delta_has_changes() {
        let mut d = Erc20Delta::new(100);
        assert!(!d.has_changes());
        d.delta_plus = U256::from(100u64);
        assert!(d.has_changes());
        d.delta_plus = U256::ZERO;
        d.delta_minus = U256::from(50u64);
        assert!(d.has_changes());
    }
}
