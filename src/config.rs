//! Configuration and watchlist loading
//!
//! Handles loading the watchlist from a file.
//! Each line should contain one Ethereum address in hex format.

use alloy_primitives::Address;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Load a watchlist from a file.
///
/// Each line should contain one Ethereum address in hex format (with or without 0x prefix).
/// Empty lines and lines starting with '#' are ignored.
///
/// # Example file format:
/// ```
// 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb
// 0xdAC17F958D2ee523a2206206994597C13D831ec7
// # This is a comment
// ```
pub fn load_watchlist(path: &Path) -> Result<Vec<Address>> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("Failed to read watchlist file: {:?}", path))?;

    let mut addresses = Vec::new();
    for (line_num, line) in contents.lines().enumerate() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse address
        let addr = parse_address(line).with_context(|| {
            format!(
                "Invalid address on line {}: {}",
                line_num + 1,
                line
            )
        })?;

        addresses.push(addr);
    }

    if addresses.is_empty() {
        anyhow::bail!("Watchlist is empty (no valid addresses found)");
    }

    Ok(addresses)
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

/// Parse an address from a hex string.
///
/// Accepts addresses with or without 0x prefix.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_watchlist() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap();
        writeln!(file, "# This is a comment").unwrap();
        writeln!(file, "").unwrap();
        writeln!(file, "0xdAC17F958D2ee523a2206206994597C13D831ec7").unwrap();
        file.flush().unwrap();

        let addresses = load_watchlist(file.path()).unwrap();
        assert_eq!(addresses.len(), 2);
    }

    #[test]
    fn test_load_watchlist_empty() {
        let file = NamedTempFile::new().unwrap();
        let result = load_watchlist(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_address() {
        let addr1 = parse_address("0x0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap();
        let addr2 = parse_address("0742d35Cc6634C0532925a3b844Bc9e7595f0bEb").unwrap();
        assert_eq!(addr1, addr2);
    }
}
