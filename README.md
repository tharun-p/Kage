# Kage - Ethereum State Store

A persistent key-value store for Ethereum state data, built with Rust and RocksDB. This is the first component of an Ethereum transaction simulation project.

## Features

- **Persistent Storage**: Uses RocksDB with column families for efficient organization
- **Ethereum Types**: Built on `alloy-primitives` for type safety
- **Deterministic Encoding**: Binary keys and values for consistent storage
- **Developer-Friendly CLI**: Simple commands with JSON output
- **Well-Tested**: Comprehensive unit tests for all core behaviors

## Project Structure

```
Kage/
├── Cargo.toml          # Project dependencies
├── README.md           # This file
└── src/
    ├── main.rs         # CLI entry point (statectl)
    ├── lib.rs          # Library root
    ├── store.rs        # StateStore trait and RocksStateStore implementation
    ├── records.rs      # AccountRecord, HeaderRecord structs
    ├── keys.rs         # Key encoding/decoding helpers
    └── cli.rs          # CLI command parsing and execution
```

## Database Schema

The store uses RocksDB with 6 column families:

- **accounts**: Account records (nonce, balance, code_hash)
- **code**: Contract bytecode by code hash
- **storage**: Storage slot values by (address, slot)
- **headers**: Block headers by block number
- **block_hashes**: Block hashes by block number
- **meta**: Metadata (head block number, etc.)

All keys use a single-byte prefix ('A', 'C', 'S', 'H', 'B', 'M') followed by binary data.

## Building

Make sure you have Rust installed (stable toolchain):

```bash
# Install Rust if needed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the project
cargo build --release

# Run tests
cargo test
```

## Running the CLI

The `statectl` binary provides a simple interface to the state store:

```bash
# Build and run
cargo run --bin statectl -- <command>

# Or install and use directly
cargo install --path .
statectl <command>
```

### Example Commands

#### Head Block Management

```bash
# Set head block
cargo run --bin statectl -- set-head 12345

# Get head block
cargo run --bin statectl -- get-head
```

Output:
```json
{
  "head_block": 12345
}
```

#### Account Operations

```bash
# Store an account
cargo run --bin statectl -- put-account \
  0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb \
  42 \
  0xde0b6b3a7640000 \
  0x0000000000000000000000000000000000000000000000000000000000000000

# Get an account
cargo run --bin statectl -- get-account \
  0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb
```

Output:
```json
{
  "address": "0x742d35cc6634c0532925a3b844bc9e7595f0beb",
  "account": {
    "nonce": 42,
    "balance": "0xde0b6b3a7640000",
    "code_hash": "0x0000000000000000000000000000000000000000000000000000000000000000"
  }
}
```

#### Code Operations

```bash
# Store contract bytecode
cargo run --bin statectl -- put-code \
  0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef \
  0x6000600052

# Get contract bytecode
cargo run --bin statectl -- get-code \
  0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef
```

#### Storage Operations

```bash
# Store a storage slot
cargo run --bin statectl -- put-storage \
  0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb \
  0x0000000000000000000000000000000000000000000000000000000000000000 \
  0x0000000000000000000000000000000000000000000000000000000000000042

# Get a storage slot (returns 0 if missing)
cargo run --bin statectl -- get-storage \
  0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb \
  0x0000000000000000000000000000000000000000000000000000000000000000
```

#### Block Header Operations

```bash
# Store a block header
cargo run --bin statectl -- put-header \
  12345 \
  1609459200 \
  0x3b9aca00 \
  0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb \
  0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890 \
  30000000 \
  1

# Get a block header
cargo run --bin statectl -- get-header 12345
```

#### Block Hash Operations

```bash
# Store a block hash
cargo run --bin statectl -- put-block-hash \
  67890 \
  0xfedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321

# Get a block hash
cargo run --bin statectl -- get-block-hash 67890
```

### Database Path

By default, the database is stored in `./state_db`. You can specify a different path:

```bash
cargo run --bin statectl -- --db-path /path/to/db get-head
```

## Using as a Library

```rust
use kage::{RocksStateStore, StateStore, AccountRecord};
use alloy_primitives::{address, b256, U256};

// Open the store
let store = RocksStateStore::open("./state_db")?;

// Store an account
let addr = address!("742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
let account = AccountRecord {
    nonce: 42,
    balance: U256::from(1000000000000000000u64),
    code_hash: b256!("0000000000000000000000000000000000000000000000000000000000000000"),
};
store.put_account(addr, &account)?;

// Retrieve the account
let retrieved = store.get_account(addr)?;
```

## Testing

Run all tests:

```bash
cargo test
```

Run tests with output:

```bash
cargo test -- --nocapture
```

The test suite verifies:
- Missing storage returns `U256::ZERO`
- Account put/get roundtrip
- Code put/get roundtrip
- Header put/get roundtrip
- Block hash put/get roundtrip
- Head block set/get

## Design Decisions

- **Error Handling**: All operations return `Result<T, anyhow::Error>` with descriptive messages
- **Serialization**: Postcard for structs (compact, deterministic), fixed 32-byte BE for U256/B256
- **Key Format**: Single-byte prefix followed by binary data for lexicographic ordering
- **Storage Semantics**: Missing storage slots return `U256::ZERO` (Ethereum convention)
- **No Unwraps**: All error cases are handled explicitly (except in tests)

## Dependencies

- `alloy-primitives`: Ethereum types (Address, B256, U256)
- `rocksdb`: Persistent key-value database
- `postcard`: Binary serialization for structs
- `anyhow`: Error handling
- `clap`: CLI argument parsing
- `serde`: Serialization framework
- `hex`: Hex string parsing

## Watcher (EOA Monitoring)

The watcher monitors finalized Ethereum blocks and updates local state for watched EOA addresses.

### Features

- Monitors finalized blocks via JSON-RPC
- Filters EOA→EOA ETH transfers
- Updates balances and nonces with correct gas/fee accounting
- Handles both legacy and EIP-1559 transactions
- Caches contract detection to minimize RPC calls

### Building the Watcher

```bash
cargo build --release --bin watcher
```

### Running the Watcher

```bash
# Basic usage (uses defaults)
cargo run --bin watcher

# With custom options
cargo run --bin watcher -- \
  --rpc-url https://eth.llamarpc.com \
  --watchlist watchlist.txt \
  --db-path ./state_db
```

### Watchlist Format

Create a `watchlist.txt` file with one Ethereum address per line:

```
0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb
0xdAC17F958D2ee523a2206206994597C13D831ec7
# Comments start with #
```

### How It Works

1. **Initialization**: On first run, the watcher:
   - Loads addresses from `watchlist.txt`
   - Fetches current balance and nonce for each address at the finalized block
   - Stores initial state in the database
   - Sets the head block to the current finalized block

2. **Monitoring Loop**: Every 12 seconds, the watcher:
   - Checks for new finalized blocks
   - Processes blocks sequentially from `local_head + 1` to `finalized_head`
   - For each block:
     - Fetches full block with transactions
     - Filters for EOA→EOA transfers affecting watched addresses
     - Fetches receipts when needed (for fee calculation)
     - Updates balances and nonces in the database
     - Updates the head block

3. **Transaction Processing**:
   - Only processes transactions where sender or receiver is in the watchlist
   - Filters for simple transfers: `to` exists, `value > 0`, `input` is empty
   - Verifies receiver is an EOA (not a contract) using cached RPC calls
   - Calculates fees correctly for both legacy and EIP-1559 transactions
   - Updates sender balance: `balance -= (value + fee)` on success, `balance -= fee` on failure
   - Updates sender nonce: always increments
   - Updates receiver balance: `balance += value` on success (only if in watchlist)

### Example Output

```
INFO Starting Ethereum address watcher
INFO RPC URL: https://eth.llamarpc.com
INFO Watchlist: watchlist.txt
INFO Database: ./state_db
INFO Initializing watcher...
INFO Loaded 2 addresses to watch
INFO Current finalized block: 18500000
INFO Initialized 0x742d35cc6634c0532925a3b844bc9e7595f0beb: balance=1000000000000000000, nonce=42
INFO Initialized 0xdac17f958d2ee523a2206206994597c13d831ec7: balance=5000000000000000000, nonce=0
INFO Initialization complete. Head set to block 18500000
INFO Starting watcher loop...
INFO New blocks available: local=18500000, finalized=18500001
INFO Processing block 18500001 (150 transactions)
INFO Completed block 18500001
```

### Testing

Run unit tests:

```bash
cargo test
```

The test suite includes:
- Fee calculation tests (legacy and EIP-1559)
- Transaction filter tests
- Contract cache tests
- Watchlist loading tests

## License

This project is part of a larger Ethereum transaction simulation system.
