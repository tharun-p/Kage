//! Contract detection cache
//!
//! In-memory cache to avoid repeated RPC calls to check if an address
//! is a contract (has code) or an EOA (externally owned account).
//!
//! Strategy:
//! - If an address has code (is_contract = true), cache it forever (contracts don't change)
//! - If an address has no code (is_contract = false), cache it for a while (could deploy later)

use alloy_primitives::Address;
use std::collections::HashMap;

/// Cache for contract detection results.
///
/// Maps addresses to whether they are contracts (true) or EOAs (false).
/// Once an address is marked as a contract, it stays cached forever.
/// EOAs are cached but could theoretically deploy code later.
pub struct ContractCache {
    /// Map of address -> is_contract (true = contract, false = EOA)
    cache: HashMap<Address, bool>,
}

impl ContractCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Check if an address is known to be a contract.
    ///
    /// Returns:
    /// - `Some(true)` if address is a contract
    /// - `Some(false)` if address is an EOA
    /// - `None` if not cached
    pub fn is_contract(&self, addr: Address) -> Option<bool> {
        self.cache.get(&addr).copied()
    }

    /// Mark an address as a contract or EOA.
    ///
    /// Once marked as a contract (true), it will be cached forever.
    /// EOAs (false) are cached but could theoretically deploy code later.
    pub fn mark_contract(&mut self, addr: Address, is_contract: bool) {
        self.cache.insert(addr, is_contract);
    }

    /// Clear the cache (useful for testing).
    #[cfg(test)]
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

impl Default for ContractCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn test_cache_operations() {
        let mut cache = ContractCache::new();
        let addr1 = address!("0000000000000000000000000000000000000001");
        let addr2 = address!("0000000000000000000000000000000000000002");

        // Initially not cached
        assert_eq!(cache.is_contract(addr1), None);

        // Mark as contract
        cache.mark_contract(addr1, true);
        assert_eq!(cache.is_contract(addr1), Some(true));

        // Mark as EOA
        cache.mark_contract(addr2, false);
        assert_eq!(cache.is_contract(addr2), Some(false));

        // Contract stays cached
        assert_eq!(cache.is_contract(addr1), Some(true));
    }
}
