//! Ethereum JSON-RPC types
//!
//! Type definitions for blocks, transactions, and receipts
//! returned from Ethereum JSON-RPC endpoints.

use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Deserializer};

/// Ethereum block with full transaction details.
#[derive(Debug, Clone, Deserialize)]
pub struct Block {
    /// Block number (hex string in JSON, parsed to u64)
    #[serde(rename = "number", deserialize_with = "deserialize_hex_u64")]
    pub number: u64,

    /// Block hash (hex string in JSON)
    #[serde(rename = "hash", deserialize_with = "deserialize_hex_b256")]
    pub hash: B256,

    /// Base fee per gas (EIP-1559, hex string in JSON)
    #[serde(rename = "baseFeePerGas", deserialize_with = "deserialize_hex_u256_opt")]
    pub base_fee_per_gas: Option<U256>,

    /// List of transactions in the block
    #[serde(rename = "transactions")]
    pub transactions: Vec<Transaction>,
}

/// Ethereum transaction.
#[derive(Debug, Clone, Deserialize)]
pub struct Transaction {
    /// Transaction hash (hex string in JSON)
    #[serde(rename = "hash", deserialize_with = "deserialize_hex_b256")]
    pub hash: B256,

    /// Sender address (hex string in JSON)
    #[serde(rename = "from", deserialize_with = "deserialize_hex_address")]
    pub from: Address,

    /// Recipient address (None for contract creation, hex string in JSON)
    #[serde(rename = "to", deserialize_with = "deserialize_hex_address_opt")]
    pub to: Option<Address>,

    /// Value transferred in wei (hex string in JSON)
    #[serde(rename = "value", deserialize_with = "deserialize_hex_u256")]
    pub value: U256,

    /// Gas price (legacy transactions, hex string in JSON)
    #[serde(rename = "gasPrice", deserialize_with = "deserialize_hex_u256_opt")]
    pub gas_price: Option<U256>,

    /// Max fee per gas (EIP-1559, hex string in JSON)
    #[serde(rename = "maxFeePerGas", deserialize_with = "deserialize_hex_u256_opt")]
    pub max_fee_per_gas: Option<U256>,

    /// Max priority fee per gas (EIP-1559, hex string in JSON)
    #[serde(rename = "maxPriorityFeePerGas", deserialize_with = "deserialize_hex_u256_opt")]
    pub max_priority_fee_per_gas: Option<U256>,

    /// Gas limit (hex string in JSON)
    #[serde(rename = "gas", deserialize_with = "deserialize_hex_u256")]
    pub gas: U256,

    /// Transaction input data (hex string in JSON, "0x" for simple transfers)
    #[serde(rename = "input", deserialize_with = "deserialize_hex_bytes")]
    pub input: Vec<u8>,

    /// Transaction nonce (hex string in JSON)
    #[serde(rename = "nonce", deserialize_with = "deserialize_hex_u64")]
    pub nonce: u64,
}

impl Transaction {
    /// Check if this is a legacy transaction (has gasPrice, no maxFeePerGas).
    pub fn is_legacy(&self) -> bool {
        self.gas_price.is_some() && self.max_fee_per_gas.is_none()
    }

    /// Check if this is an EIP-1559 transaction (has maxFeePerGas).
    pub fn is_eip1559(&self) -> bool {
        self.max_fee_per_gas.is_some()
    }

    /// Check if this is a contract creation transaction (to is None).
    pub fn is_contract_creation(&self) -> bool {
        self.to.is_none()
    }
}

/// Log entry emitted by a contract during transaction execution.
#[derive(Debug, Clone, Deserialize)]
pub struct Log {
    /// Address of the contract that emitted the log
    #[serde(rename = "address", deserialize_with = "deserialize_hex_address")]
    pub address: Address,

    /// Indexed topics (topic0 = event signature, topics[1..] = indexed params)
    #[serde(rename = "topics", default)]
    pub topics: Vec<String>,

    /// Non-indexed event data (hex string)
    #[serde(rename = "data", deserialize_with = "deserialize_hex_bytes")]
    pub data: Vec<u8>,
}

/// Transaction receipt.
#[derive(Debug, Clone, Deserialize)]
pub struct Receipt {
    /// Transaction status: 1 = success, 0 = failure (hex string in JSON)
    #[serde(rename = "status", deserialize_with = "deserialize_hex_u64")]
    pub status: u64,

    /// Gas used (hex string in JSON)
    #[serde(rename = "gasUsed", deserialize_with = "deserialize_hex_u256")]
    pub gas_used: U256,

    /// Effective gas price (post-London, hex string in JSON)
    #[serde(rename = "effectiveGasPrice", deserialize_with = "deserialize_hex_u256_opt")]
    pub effective_gas_price: Option<U256>,

    /// Logs emitted during transaction execution (empty for reverted txs)
    #[serde(rename = "logs", default)]
    pub logs: Vec<Log>,
}

impl Receipt {
    /// Check if the transaction succeeded.
    pub fn is_success(&self) -> bool {
        self.status == 1
    }

    /// Check if the transaction failed.
    pub fn is_failure(&self) -> bool {
        self.status == 0
    }
}

/// Call trace node produced by `debug_traceTransaction` with `callTracer`.
///
/// We keep this struct intentionally liberal (many optional fields) so it
/// can handle slightly different implementations across clients. For the
/// internal transfer tracking use case we only care about:
/// - `type`  (CALL / CALLCODE / STATICCALL / DELEGATECALL / SELFDESTRUCT / ...)
/// - `from`  (sender address)
/// - `to`    (receiver address, may be None for CREATE)
/// - `value` (amount of wei transferred)
/// - `calls` (nested children)
#[derive(Debug, Clone, Deserialize)]
pub struct CallTrace {
    /// Call type: CALL / STATICCALL / DELEGATECALL / CALLCODE / SELFDESTRUCT / ...
    #[serde(rename = "type")]
    pub r#type: Option<String>,

    /// Sender address (hex string in JSON, may be omitted in some edge cases).
    #[serde(default, deserialize_with = "deserialize_hex_address_opt")]
    pub from: Option<Address>,

    /// Recipient address (hex string in JSON, None for CREATE-like nodes).
    #[serde(default, deserialize_with = "deserialize_hex_address_opt")]
    pub to: Option<Address>,

    /// Value transferred in wei (hex string in JSON).
    ///
    /// Missing or empty values are treated as zero for robustness.
    #[serde(default, deserialize_with = "deserialize_hex_u256_trace")]
    pub value: U256,

    /// Nested child calls.
    #[serde(default)]
    pub calls: Option<Vec<CallTrace>>,

    /// Optional error / revert reason field used by some clients.
    #[serde(default)]
    pub error: Option<String>,
}

// Hex deserialization helpers

/// Pad an odd-length hex string with a leading zero.
/// This handles cases where RPC returns hex strings without leading zeros.
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

/// Deserialize a hex string to u64.
fn deserialize_hex_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let s = s.strip_prefix("0x").unwrap_or(&s);
    u64::from_str_radix(s, 16).map_err(serde::de::Error::custom)
}

/// Deserialize a hex string to U256.
fn deserialize_hex_u256<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let s = s.strip_prefix("0x").unwrap_or(&s);
    if s.is_empty() {
        return Ok(U256::ZERO);
    }
    let s = pad_hex_string(s);
    let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
    Ok(U256::from_be_slice(&bytes))
}

/// Deserialize an optional hex string to U256.
fn deserialize_hex_u256_opt<'de, D>(deserializer: D) -> Result<Option<U256>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = Option::<String>::deserialize(deserializer)?;
    match s {
        Some(s) => {
            let s = s.strip_prefix("0x").unwrap_or(&s);
            if s.is_empty() {
                Ok(Some(U256::ZERO))
            } else {
                let s = pad_hex_string(&s);
                let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
                Ok(Some(U256::from_be_slice(&bytes)))
            }
        }
        None => Ok(None),
    }
}

/// Deserialize a hex string to B256.
fn deserialize_hex_b256<'de, D>(deserializer: D) -> Result<B256, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let s = s.strip_prefix("0x").unwrap_or(&s);
    let s = pad_hex_string(&s);
    let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
    if bytes.len() != 32 {
        return Err(serde::de::Error::custom(format!(
            "Expected 32 bytes for hash, got {}",
            bytes.len()
        )));
    }
    Ok(B256::from_slice(&bytes))
}

/// Deserialize a hex string to Address.
fn deserialize_hex_address<'de, D>(deserializer: D) -> Result<Address, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let s = s.strip_prefix("0x").unwrap_or(&s);
    let s = pad_hex_string(&s);
    let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
    if bytes.len() != 20 {
        return Err(serde::de::Error::custom(format!(
            "Expected 20 bytes for address, got {}",
            bytes.len()
        )));
    }
    Ok(Address::from_slice(&bytes))
}

/// Deserialize an optional hex string to Address.
fn deserialize_hex_address_opt<'de, D>(deserializer: D) -> Result<Option<Address>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = Option::<String>::deserialize(deserializer)?;
    match s {
        Some(s) => {
            let s = s.strip_prefix("0x").unwrap_or(&s);
            if s.is_empty() {
                Ok(None)
            } else {
                let s = pad_hex_string(&s);
                let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
                if bytes.len() != 20 {
                    return Err(serde::de::Error::custom(format!(
                        "Expected 20 bytes for address, got {}",
                        bytes.len()
                    )));
                }
                Ok(Some(Address::from_slice(&bytes)))
            }
        }
        None => Ok(None),
    }
}

/// Deserialize a hex string to bytes.
fn deserialize_hex_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let s = s.strip_prefix("0x").unwrap_or(&s);
    if s.is_empty() {
        Ok(Vec::new())
    } else {
        let s = pad_hex_string(&s);
        hex::decode(&s).map_err(serde::de::Error::custom)
    }
}

/// Deserialize a hex string (or null / missing) to U256 for trace values.
///
/// This variant is a bit more forgiving than `deserialize_hex_u256`:
/// - null / missing ⇒ 0
/// - empty string  ⇒ 0
fn deserialize_hex_u256_trace<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    let s = Option::<String>::deserialize(deserializer)?;
    match s {
        Some(s) => {
            let s = s.strip_prefix("0x").unwrap_or(&s);
            if s.is_empty() {
                Ok(U256::ZERO)
            } else {
                let s = pad_hex_string(&s);
                let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
                Ok(U256::from_be_slice(&bytes))
            }
        }
        None => Ok(U256::ZERO),
    }
}
