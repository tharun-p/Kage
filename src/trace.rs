//! Call trace utilities for internal ETH transfers
//!
//! This module understands the output of the `callTracer` debug tracer and
//! provides helpers to extract internal ETH transfers that we care about:
//! contract → watched EOA credits that actually moved value.
//!
//! Important guardrails:
//! - We never double-count EOA→EOA top-level transfers because we only
//!   consider nodes where the *sender is a contract* (determined by the
//!   caller via a predicate).
//! - We ignore DELEGATECALL / STATICCALL nodes even if they report a value.
//! - We treat SELFDESTRUCT-like nodes as value transfers from the contract
//!   to the beneficiary if present.

use crate::types::CallTrace;
use alloy_primitives::{Address, U256};
use std::collections::HashSet;

/// Internal ETH transfer discovered from a call trace.
///
/// This only represents *credits* to watched EOAs. We deliberately do
/// not model contract debits here – contracts are out of scope for the
/// active state store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalTransfer {
    /// Contract address that sent ETH.
    pub from: Address,
    /// Watched EOA that received ETH.
    pub to: Address,
    /// Amount of wei received.
    pub value: U256,
}

/// Collect internal transfers from a call trace.
///
/// - `trace`: root of the `callTracer` call tree.
/// - `tx_succeeded`: whether the *overall* transaction succeeded
///   (`receipt.status == 1`). If false, no transfers are returned.
/// - `watchlist`: set of watched EOAs to track credits for.
/// - `sender_is_contract`: predicate used to decide whether a given
///   address is a contract. The caller is responsible for caching /
///   RPC lookups as needed.
///
/// Returns a list of contract → watched EOA credits that actually
/// moved value (value > 0).
pub fn collect_internal_transfers<F>(
    trace: &CallTrace,
    tx_succeeded: bool,
    watchlist: &HashSet<Address>,
    mut sender_is_contract: F,
) -> Vec<InternalTransfer>
where
    F: FnMut(Address) -> bool,
{
    let mut result = Vec::new();

    // Guardrail: if the overall transaction reverted, we ignore *all*
    // trace-derived transfers. The EVM state changes were rolled back.
    if !tx_succeeded {
        return result;
    }

    fn is_value_transfer_node(node_type: &str) -> bool {
        // We treat CALL, CALLCODE, and SELFDESTRUCT as potential value
        // transfers. We *ignore* STATICCALL and DELEGATECALL even if a
        // buggy tracer were to report a non-zero value.
        let t = node_type.to_ascii_uppercase();
        matches!(t.as_str(), "CALL" | "CALLCODE" | "SELFDESTRUCT")
    }

    fn walk<F>(
        node: &CallTrace,
        watchlist: &HashSet<Address>,
        sender_is_contract: &mut F,
        out: &mut Vec<InternalTransfer>,
    ) where
        F: FnMut(Address) -> bool,
    {
        let node_type = node.r#type.as_deref().unwrap_or("");

        if is_value_transfer_node(node_type) {
            if let (Some(from), Some(to)) = (node.from, node.to) {
                if node.value > U256::ZERO
                    && sender_is_contract(from)
                    && watchlist.contains(&to)
                {
                    out.push(InternalTransfer { from, to, value: node.value });
                }
            }
        }

        if let Some(children) = &node.calls {
            for child in children {
                walk(child, watchlist, sender_is_contract, out);
            }
        }
    }

    walk(trace, watchlist, &mut sender_is_contract, &mut result);
    result
}

/// Collect all unique senders that appear in a trace.
///
/// This is useful for pre-populating a contract/EOA cache before
/// filtering transfers, so that `sender_is_contract` can be pure and
/// synchronous.
pub fn collect_senders(trace: &CallTrace) -> HashSet<Address> {
    let mut set = HashSet::new();

    fn walk(node: &CallTrace, set: &mut HashSet<Address>) {
        if let Some(from) = node.from {
            set.insert(from);
        }
        if let Some(children) = &node.calls {
            for child in children {
                walk(child, set);
            }
        }
    }

    walk(trace, &mut set);
    set
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, U256};

    fn addr1() -> Address {
        address!("0000000000000000000000000000000000000001")
    }
    fn addr2() -> Address {
        address!("0000000000000000000000000000000000000002")
    }
    fn addr3() -> Address {
        address!("0000000000000000000000000000000000000003")
    }

    /// Helper to build a simple call trace node.
    fn node(
        node_type: &str,
        from: Option<Address>,
        to: Option<Address>,
        value: U256,
        calls: Option<Vec<CallTrace>>,
    ) -> CallTrace {
        CallTrace {
            r#type: Some(node_type.to_string()),
            from,
            to,
            value,
            calls,
            error: None,
        }
    }

    #[test]
    fn test_contract_to_watched_eoa_nested_call() {
        // Top-level: EOA -> contract (no value)
        // Nested:    contract -> watched EOA (value > 0)
        let eoa = addr1();
        let contract = addr2();
        let watched = addr3();

        let inner = node("CALL", Some(contract), Some(watched), U256::from(1000u64), None);
        let root = node("CALL", Some(eoa), Some(contract), U256::ZERO, Some(vec![inner]));

        let mut watchlist = HashSet::new();
        watchlist.insert(watched);

        // Only `contract` is a contract.
        let transfers = collect_internal_transfers(&root, true, &watchlist, |from| from == contract);

        assert_eq!(transfers.len(), 1);
        let t = &transfers[0];
        assert_eq!(t.from, contract);
        assert_eq!(t.to, watched);
        assert_eq!(t.value, U256::from(1000u64));
    }

    #[test]
    fn test_eoa_to_watched_eoa_not_counted() {
        // EOA -> watched EOA with value > 0 should NOT be counted here,
        // because EOA→EOA top-level transfers are handled elsewhere.
        let eoa = addr1();
        let watched = addr3();

        let root = node("CALL", Some(eoa), Some(watched), U256::from(500u64), None);

        let mut watchlist = HashSet::new();
        watchlist.insert(watched);

        let transfers = collect_internal_transfers(&root, true, &watchlist, |_from| false);
        assert!(transfers.is_empty());
    }

    #[test]
    fn test_reverted_tx_returns_no_transfers() {
        let contract = addr2();
        let watched = addr3();

        let root = node("CALL", Some(contract), Some(watched), U256::from(123u64), None);

        let mut watchlist = HashSet::new();
        watchlist.insert(watched);

        // tx_succeeded = false ⇒ no transfers
        let transfers = collect_internal_transfers(&root, false, &watchlist, |_from| true);
        assert!(transfers.is_empty());
    }

    #[test]
    fn test_delegatecall_ignored_even_with_value() {
        let contract = addr2();
        let watched = addr3();

        let root = node(
            "DELEGATECALL",
            Some(contract),
            Some(watched),
            U256::from(999u64),
            None,
        );

        let mut watchlist = HashSet::new();
        watchlist.insert(watched);

        let transfers = collect_internal_transfers(&root, true, &watchlist, |_from| true);
        assert!(transfers.is_empty());
    }

    #[test]
    fn test_selfdestruct_like_node_counted() {
        // Some tracers may represent SELFDESTRUCT payouts explicitly.
        let contract = addr2();
        let watched = addr3();

        let root = node(
            "SELFDESTRUCT",
            Some(contract),
            Some(watched),
            U256::from(777u64),
            None,
        );

        let mut watchlist = HashSet::new();
        watchlist.insert(watched);

        let transfers = collect_internal_transfers(&root, true, &watchlist, |_from| true);
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].from, contract);
        assert_eq!(transfers[0].to, watched);
        assert_eq!(transfers[0].value, U256::from(777u64));
    }
}

