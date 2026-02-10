# Kage - Ethereum State Store & Transaction Tracker

A comprehensive Ethereum state tracking system built with Rust and RocksDB. Tracks ETH and ERC20 token balances for watched addresses over time, with efficient sparse storage and fill-forward queries.

## Features

- **Persistent Storage**: Uses RocksDB with column families for efficient organization
- **ETH Balance Tracking**: Monitors EOA balances and nonces with correct gas/fee accounting
- **ERC20 Token Tracking**: Tracks ERC20 token balances via Transfer event parsing
- **Internal Transfer Detection**: Uses transaction tracing to detect contract→EOA ETH transfers
- **Sparse Storage**: Only stores changes (deltas) and periodic snapshots for efficiency
- **Fill-Forward Queries**: Reconstructs dense balance history from sparse data
- **Coverage Tracking**: Prevents queries before tracking started (watch_start_block)
- **Modular Tracker System**: Extensible pipeline for future protocols (Uniswap, Aave, etc.)
- **Developer-Friendly CLI**: Simple commands with JSON output and coverage metadata
- **Well-Tested**: Comprehensive unit tests for all core behaviors

## Architecture Diagrams

> **Note**: Interactive Mermaid diagrams are available in [`docs/diagrams/`](docs/diagrams/). The diagrams below are ASCII art versions viewable in any markdown previewer.

### System Architecture Overview

**Mermaid version**: [`docs/diagrams/system-architecture.md`](docs/diagrams/system-architecture.md)

```
┌─────────────────────────────────────────────────────────────────┐
│                        Entry Points                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐                    ┌──────────────┐          │
│  │ statectl CLI │                    │ watcher      │          │
│  │              │                    │ binary       │          │
│  └──────┬───────┘                    └──────┬───────┘          │
│         │                                    │                  │
└─────────┼────────────────────────────────────┼──────────────────┘
          │                                    │
          │                                    ▼
          │                          ┌─────────────────┐
          │                          │ Watcher         │
          │                          │ Orchestrator    │
          │                          └────────┬────────┘
          │                                   │
          │              ┌────────────────────┼────────────────────┐
          │              │                    │                    │
          ▼              ▼                    ▼                    ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ RocksState   │  │ RPC Client   │  │ Contract     │  │ Transaction │
│ Store        │  │              │  │ Cache        │  │ Apply Logic │
└──────┬───────┘  └──────┬───────┘  └──────────────┘  └──────┬──────┘
       │                 │                                    │
       │                 │                                    │
       │                 ▼                                    │
       │          ┌──────────────┐                           │
       │          │ Ethereum     │                           │
       │          │ Node (RPC)   │                           │
       │          └──────────────┘                           │
       │                                                      │
       ▼                                                      │
┌──────────────┐                                             │
│ RocksDB      │◄────────────────────────────────────────────┘
│ (13 CFs)     │
└──────────────┘

Processing Pipeline:
  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
  │ Trace        │  │ ETH Tracker  │  │ ERC20        │  │ (Future:     │
  │ Parser       │  │ (Built-in)   │  │ Tracker      │  │  Uniswap,    │
  │              │  │              │  │ (Plugin)     │  │  Aave, etc.) │
  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘
```

### Data Flow Diagram

**Mermaid version**: [`docs/diagrams/data-flow.md`](docs/diagrams/data-flow.md)

```
┌──────┐
│ User │
└──┬───┘
   │ Start watcher
   ▼
┌─────────┐     ┌──────────┐     ┌──────────────┐     ┌──────────┐
│ Watcher │────▶│ RPC      │────▶│ Ethereum     │     │ Store    │
│         │◀────│ Client   │◀────│ Node         │     │          │
└────┬────┘     └──────────┘     └──────────────┘     └────┬─────┘
     │                                                       │
     │ Initialization:                                      │
     │   1. Load watchlist                                  │
     │   2. Get latest block                                │
     │   3. For each address:                               │
     │      - Get balance & nonce                           │
     │      - Store snapshot + WatchMeta                    │
     │                                                       │
     │ Poll Loop (every 12s):                               │
     │   ┌─────────────────────────────────────┐           │
     │   │ For each new block:                 │           │
     │   │   1. Fetch full block               │           │
     │   │   2. For each transaction:          │           │
     │   │      - Fetch receipt                │           │
     │   │      - If successful:               │           │
     │   │        * Trace (internal transfers) │           │
     │   │        * Apply ETH changes          │           │
     │   │      - Parse ERC20 Transfer events  │           │
     │   │   3. Persist deltas & snapshots     │           │
     │   │   4. Update head block              │           │
     │   └─────────────────────────────────────┘           │
     │                                                       │
     │ Query Flow:                                          │
     │   User → CLI → Store → DB                            │
     │   Store: Find anchor → Load deltas → Fill-forward    │
     │   Return: Dense balance list with metadata           │
     └───────────────────────────────────────────────────────┘
```

### Component Interaction Diagram

**Mermaid version**: [`docs/diagrams/component-interaction.md`](docs/diagrams/component-interaction.md)

```
┌──────────────────────────────────────────────────────────────────┐
│                         Watcher Module                           │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────┐                                                    │
│  │ Watcher  │                                                    │
│  └────┬─────┘                                                    │
│       │                                                          │
│       ├──▶ Initialize ──┐                                       │
│       ├──▶ Poll Loop     │                                       │
│       └──▶ Process Block │                                       │
│                          │                                       │
└──────────┬───────────────┼───────────────────────────────────────┘
           │               │
           │               ▼
           │        ┌──────────────┐
           │        │ Config       │
           │        │ Loader       │
           │        └──────────────┘
           │
           ▼
┌──────────────────────────────────────────────────────────────────┐
│                    Transaction Processing                        │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  Filter ──▶ Apply Transaction ──▶ Trace ──▶ Apply Internal       │
│              │                      │                             │
│              ├──▶ Fee Calculator    └──▶ RPC Client              │
│              └──▶ Contract Cache                                  │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
           │
           ▼
┌──────────────────────────────────────────────────────────────────┐
│                        Tracker System                            │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────────┐                                                │
│  │ Tracker      │                                                │
│  │ Trait        │                                                │
│  └──────┬───────┘                                                │
│         │                                                        │
│         ├──▶ ETH Tracker (Built-in)                             │
│         └──▶ ERC20 Tracker (Plugin)                             │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
           │
           ▼
┌──────────────────────────────────────────────────────────────────┐
│                         Storage Layer                            │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  StateStore Trait ──▶ RocksStateStore ──▶ Key Encoder/Decoder     │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### Database Structure Diagram

**Mermaid version**: [`docs/diagrams/database-structure.md`](docs/diagrams/database-structure.md)

```
┌──────────────────────────────────────────────────────────────────┐
│                           RocksDB                                │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  Core State:                                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ accounts    │  │ code        │  │ storage     │             │
│  │ Key: A+addr │  │ Key: C+hash │  │ Key: S+addr+│             │
│  │ Value:      │  │ Value: bytes│  │     slot    │             │
│  │ AccountRec  │  │             │  │ Value: U256  │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
│                                                                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ headers     │  │ block_hashes│  │ meta        │             │
│  │ Key: H+block│  │ Key: B+block│  │ Key: M+id   │             │
│  │ Value:      │  │ Value: B256  │  │ Value: u64  │             │
│  │ HeaderRec   │  │             │  │             │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
│                                                                   │
│  ETH Tracking:                                                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ block_      │  │ balance_    │  │ watch_meta │             │
│  │ deltas      │  │ snapshots    │  │             │             │
│  │ Key: D+addr │  │ Key: Z+addr │  │ Key: W+addr │             │
│  │     +block  │  │     +block  │  │ Value:      │             │
│  │ Value:      │  │ Value: U256 │  │ WatchMeta   │             │
│  │ BlockDelta  │  │             │  │             │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
│                                                                   │
│  ERC20 Tracking:                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ erc20_      │  │ erc20_      │  │ erc20_      │             │
│  │ deltas      │  │ snapshots   │  │ watch_meta  │             │
│  │ Key: T+token│  │ Key: U+token│  │ Key: X+token│             │
│  │     +owner  │  │     +owner  │  │     +owner  │             │
│  │     +block  │  │     +block  │  │ Value:      │             │
│  │ Value:      │  │ Value: U256 │  │ TokenWatch  │             │
│  │ Erc20Delta  │  │             │  │ Meta        │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
│                                                                   │
│  ┌─────────────┐                                                 │
│  │ erc20_      │                                                 │
│  │ balances    │                                                 │
│  │ Key: X+token│                                                 │
│  │     +owner  │                                                 │
│  │ Value: U256 │                                                 │
│  │ (current)   │                                                 │
│  └─────────────┘                                                 │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### Processing Pipeline Flow

**Mermaid version**: [`docs/diagrams/processing-pipeline.md`](docs/diagrams/processing-pipeline.md)

```
┌──────────────────────────────────────────────────────────────────┐
│                         INITIALIZATION                           │
└──────────────────────────────────────────────────────────────────┘
                    │
                    ▼
        ┌───────────────────────┐
        │ Watcher Starts        │
        └───────────┬───────────┘
                    │
                    ▼
        ┌───────────────────────┐
        │ Initialize            │
        │  • Load watchlist.txt │
        │  • Load tokens.txt    │
        │  • Fetch balances     │
        │  • Store snapshots    │
        │  • Set head block     │
        └───────────┬───────────┘
                    │
                    ▼
┌──────────────────────────────────────────────────────────────────┐
│                         POLL LOOP                                │
│                      (Every 12 seconds)                          │
└──────────────────────────────────────────────────────────────────┘
                    │
                    ▼
        ┌───────────────────────┐
        │ New blocks            │
        │ available?            │
        └───────┬───────────────┘
                │
        ┌───────┴───────┐
        │               │
       Yes              No
        │               │
        ▼               ▼
┌──────────────┐  ┌──────────┐
│ Fetch Block  │  │ Wait 12s │
└──────┬───────┘  └────┬─────┘
       │                │
       │                └──────┐
       │                       │
       ▼                       │
┌───────────────────────┐      │
│ Process Block         │      │
│  • Fetch transactions │      │
│  • For each tx:       │      │
│    - Fetch receipt    │      │
│    - If successful:    │      │
│      * Trace          │      │
│      * Apply ETH      │      │
│    - Parse ERC20 logs │      │
│  • Persist deltas    │      │
│  • Update head        │      │
└───────┬───────────────┘      │
        │                      │
        └──────────────────────┘
```

### Query Flow Diagram

**Mermaid version**: [`docs/diagrams/query-flow.md`](docs/diagrams/query-flow.md)

```
┌──────┐
│ User │ Query balances/deltas
└──┬───┘
   │
   ▼
┌──────────┐
│ statectl │ get_balances_in_range_with_metadata
└────┬─────┘
     │
     ▼
┌──────────────────────────────────────────────────────────────────┐
│                      StateStore                                   │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  1. Get WatchMeta                                                │
│     └─▶ Read watch_meta CF → { start_block }                     │
│                                                                   │
│  2. Clamp Query Range                                            │
│     effective_start = max(requested_start, watch_start_block)   │
│     effective_end = min(requested_end, head_block)               │
│                                                                   │
│  3. Find Anchor Snapshot                                         │
│     └─▶ Seek latest snapshot ≤ effective_start                   │
│         Validate: snapshot.block >= watch_start_block             │
│                                                                   │
│  4. Load Deltas in Range                                         │
│     └─▶ Iterate deltas [effective_start, effective_end]          │
│                                                                   │
│  5. Fill-Forward Algorithm                                       │
│     balance = anchor.balance                                      │
│     For each block in range:                                     │
│       if delta exists:                                            │
│         balance += delta_plus - delta_minus                       │
│       else:                                                       │
│         carry forward (balance unchanged)                        │
│       push (block, balance)                                       │
│                                                                   │
│  6. Build QueryResult                                            │
│     • requested_start/end                                        │
│     • effective_start/end                                        │
│     • watch_start_block                                          │
│     • head_block                                                 │
│     • message (if clamped)                                       │
│     • data: Vec<(block, balance)>                                │
│                                                                   │
└────┬──────────────────────────────────────────────────────────────┘
     │
     ▼
┌──────────┐
│ statectl │ Pretty JSON output
└────┬─────┘
     │
     ▼
┌──────┐
│ User │
└──────┘
```

## Project Structure

```
Kage/
├── Cargo.toml          # Project dependencies
├── README.md           # This file
├── watchlist.txt       # EOA addresses to monitor
├── tokens.txt          # ERC20 token addresses to monitor (optional)
└── src/
    ├── main.rs         # CLI entry point (statectl)
    ├── watcher_main.rs # Watcher binary entry point
    ├── lib.rs          # Library root
    ├── store.rs        # StateStore trait and RocksStateStore implementation
    ├── records.rs      # Data structures (AccountRecord, BlockDelta, Erc20Delta, etc.)
    ├── keys.rs         # Key encoding/decoding helpers
    ├── cli.rs          # CLI command parsing and execution
    ├── watcher.rs      # Main block processing orchestrator
    ├── rpc.rs          # Ethereum JSON-RPC client
    ├── apply.rs        # Transaction application logic
    ├── fee.rs          # Gas fee calculation
    ├── trace.rs        # Transaction trace parsing for internal transfers
    ├── tracker.rs      # Tracker trait and context
    ├── tracker_erc20.rs # ERC20 Transfer event tracker
    ├── cache.rs        # Contract/EOA detection cache
    ├── config.rs       # Watchlist loading
    └── types.rs        # JSON-RPC type definitions
```

## Database Schema

The store uses RocksDB with 13 column families:

### Core State
- **accounts**: Account records (nonce, balance, code_hash)
- **code**: Contract bytecode by code hash
- **storage**: Storage slot values by (address, slot)
- **headers**: Block headers by block number
- **block_hashes**: Block hashes by block number
- **meta**: Metadata (head block number, etc.)

### ETH Tracking
- **block_deltas**: Sparse ETH balance changes per (address, block)
- **balance_snapshots**: Sparse ETH balance snapshots per (address, block)
- **watch_meta**: Coverage metadata (start_block per address)

### ERC20 Tracking
- **erc20_deltas**: Sparse ERC20 token changes per (token, owner, block)
- **erc20_snapshots**: Sparse ERC20 token snapshots per (token, owner, block)
- **erc20_watch_meta**: ERC20 coverage metadata (start_block per token, owner)
- **erc20_balances**: Current ERC20 balances for fast lookup

### Key Format

All keys use a single-byte prefix followed by binary data for lexicographic ordering:

- `'A'` + address(20) → Account
- `'D'` + address(20) + block(u64 BE) → ETH Delta (address-first for prefix scans)
- `'Z'` + address(20) + block(u64 BE) → ETH Snapshot
- `'W'` + address(20) → Watch Metadata
- `'T'` + token(20) + owner(20) + block(u64 BE) → ERC20 Delta
- `'U'` + token(20) + owner(20) + block(u64 BE) → ERC20 Snapshot
- `'X'` + token(20) + owner(20) → Token Watch Metadata

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

## Running the Watcher

The watcher monitors Ethereum blocks and updates local state for watched addresses.

### Basic Usage

```bash
# Run with defaults (localhost RPC, watchlist.txt, ./state_db)
cargo run --bin watcher

# With custom options
cargo run --bin watcher -- \
  --rpc-url http://127.0.0.1:8545 \
  --watchlist watchlist.txt \
  --tokens tokens.txt \
  --db-path ./state_db
```

### Watchlist Format

Create a `watchlist.txt` file with one Ethereum address per line:

```
0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb
0x70997970C51812dc3A010C7d01b50e0d17dc79C8
# Comments start with #
```

### Token Watchlist Format

Create a `tokens.txt` file with one ERC20 token contract address per line:

```
# ERC20 token watchlist
0x6e989C01a3e3A94C973A62280a72EC335598490e
```

### How It Works

1. **Initialization**: On first run, the watcher:
   - Loads EOA addresses from `watchlist.txt`
   - Optionally loads ERC20 tokens from `tokens.txt`
   - Fetches current ETH balance and nonce for each address at "latest" block
   - For each (token, owner) pair, calls `balanceOf` to get initial ERC20 balance
   - Stores initial snapshots and `WatchMeta`/`TokenWatchMeta` with `start_block`
   - Sets the head block to the current block

2. **Monitoring Loop**: Every 12 seconds, the watcher:
   - Checks for new blocks (uses "latest" for compatibility with Anvil)
   - Processes blocks sequentially from `local_head + 1` to `latest`
   - For each block:
     - Fetches full block with transactions
     - Processes transactions:
       - **Top-level ETH transfers**: Filters EOA→EOA transfers, updates balances/fees/nonce
       - **Internal transfers**: Uses `debug_traceTransaction` to detect contract→EOA ETH transfers
       - **ERC20 transfers**: Parses `Transfer` events from receipts, updates token balances
     - Persists deltas and snapshots for changed addresses
     - Updates the head block

3. **Transaction Processing**:
   - **ETH Transfers**: Only processes transactions where sender or receiver is in watchlist
   - Filters for simple transfers: `to` exists, `value > 0`, `input` is empty
   - Verifies receiver is an EOA (not a contract) using cached RPC calls
   - Calculates fees correctly for both legacy and EIP-1559 transactions
   - Updates sender balance: `balance -= (value + fee)` on success, `balance -= fee` on failure
   - Updates sender nonce: always increments
   - Updates receiver balance: `balance += value` on success (only if in watchlist)

4. **ERC20 Processing**:
   - Parses `Transfer(address,address,uint256)` events from transaction receipts
   - Only processes logs from watched tokens
   - Only updates balances for watched owners
   - Handles mint (from=0x0), burn (to=0x0), and normal transfers
   - Ignores logs from reverted transactions
   - Persists deltas and snapshots per (token, owner, block)

### Example Output

```
INFO Starting Ethereum address watcher
INFO RPC URL: http://127.0.0.1:8545
INFO Watchlist: watchlist.txt
INFO Database: ./state_db
INFO Initializing watcher...
INFO Loaded 2 addresses to watch
INFO Loaded 1 tokens to watch
INFO Initialized 0x742d35cc6634c0532925a3b844bc9e7595f0beb: balance=1000000000000000000, nonce=42
INFO Initialized ERC20 token 0x6e98... for owner 0x742d...: balance=500
INFO Initialization complete. Head set to block 100
INFO Starting watcher loop...
INFO New blocks available: local=100, latest=101
INFO Processing block 101 (5 transactions)
INFO Completed block 101 (2 addresses changed, traced_tx_count=3, internal_credits=1)
```

## CLI Commands

The `statectl` binary provides a simple interface to query the state store:

### Basic Commands

```bash
# Set/get head block
cargo run --bin statectl -- set-head 12345
cargo run --bin statectl -- get-head

# Account operations
cargo run --bin statectl -- put-account <address> <nonce> <balance_hex> <code_hash>
cargo run --bin statectl -- get-account <address>

# Code operations
cargo run --bin statectl -- put-code <code_hash> <hex_bytecode>
cargo run --bin statectl -- get-code <code_hash>

# Storage operations
cargo run --bin statectl -- put-storage <address> <slot> <value_hex>
cargo run --bin statectl -- get-storage <address> <slot>
```

### Balance & Delta Queries

#### ETH Balances (with fill-forward)

```bash
cargo run --bin statectl -- balances <address> <start_block> <end_block>
```

Example:
```bash
cargo run --bin statectl -- balances 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb 100 105
```

Output:
```json
{
  "address": "0x742d35cc6634c0532925a3b844bc9e7595f0beb",
  "requestedStart": 100,
  "requestedEnd": 105,
  "effectiveStart": 100,
  "effectiveEnd": 105,
  "watchStartBlock": 100,
  "headBlock": 150,
  "message": null,
  "balances": [
    { "block": 100, "balance": "0xde0b6b3a7640000" },
    { "block": 101, "balance": "0xde0b6b3a7640000" },
    { "block": 102, "balance": "0xde0b6b3a7640000" },
    { "block": 103, "balance": "0xde0b6b3a7640000" },
    { "block": 104, "balance": "0xde0b6b3a7640000" },
    { "block": 105, "balance": "0xde0b6b3a7640000" }
  ]
}
```

#### ETH Deltas (sparse or dense)

```bash
# Sparse (only blocks with changes)
cargo run --bin statectl -- deltas <address> <start_block> <end_block>

# Dense (all blocks, zero deltas included)
cargo run --bin statectl -- deltas <address> <start_block> <end_block> --dense
```

#### ERC20 Balances

```bash
cargo run --bin statectl -- erc20-balances <token_address> <owner_address> <start_block> <end_block>
```

Example:
```bash
cargo run --bin statectl -- erc20-balances \
  0x6e989C01a3e3A94C973A62280a72EC335598490e \
  0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb \
  100 105
```

#### ERC20 Deltas

```bash
# Sparse
cargo run --bin statectl -- erc20-deltas <token_address> <owner_address> <start_block> <end_block>

# Dense
cargo run --bin statectl -- erc20-deltas <token_address> <owner_address> <start_block> <end_block> --dense
```

### Database Path

By default, the database is stored in `./state_db`. You can specify a different path:

```bash
cargo run --bin statectl -- --db-path /path/to/db balances <address> 100 105
```

## Architecture Overview

### High-Level Flow

```
┌─────────────────┐
│  watcher_main   │  Entry point (CLI)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│    Watcher      │  Orchestrates block processing
└────────┬────────┘
         │
    ┌────┴────┬──────────────┐
    ▼         ▼              ▼
┌────────┐ ┌──────┐    ┌──────────┐
│  RPC   │ │Store │    │ Trackers │
│ Client │ │(DB)  │    │ (ETH/ERC)│
└────────┘ └──────┘    └──────────┘
```

### Key Design Decisions

1. **Sparse Storage**: Only stores changes (deltas) and periodic snapshots, not every block
2. **Address-First Keys**: Enables efficient prefix scans for range queries
3. **Fill-Forward Queries**: Reconstructs dense balance history from sparse data
4. **Coverage Tracking**: Prevents queries before tracking started (watch_start_block)
5. **Modular Trackers**: Extensible pipeline for future protocols (Uniswap, Aave, etc.)
6. **Point-in-Time Initialization**: Starts tracking from current block, not genesis
7. **Contract Cache**: Avoids repeated RPC calls for contract detection
8. **Error Handling**: `anyhow::Result`, no `unwrap()` in production code

### Data Flow Example

**Scenario**: Track ETH and ERC20 balances for address `0xABC...`

1. **Initialization**:
   - Load watchlist → `[0xABC...]`
   - Load tokens → `[0x6e98...]`
   - Fetch `balance(0xABC)` at "latest" → 1000 ETH
   - Fetch `balanceOf(0x6e98, 0xABC)` → 500 tokens
   - Store snapshot at block 100: ETH=1000, tokens=500
   - Store `WatchMeta { start_block: 100 }`
   - Store `TokenWatchMeta { start_block: 100 }`

2. **Block Processing (block 101)**:
   - Transaction: `0xABC` sends 10 ETH to `0xDEF`
   - Fetch receipt → success, gas_used=21000, effective_gas_price=20 gwei
   - Apply: `0xABC` balance -= (10 ETH + fee), nonce++
   - Apply: `0xDEF` balance += 10 ETH (if watched)
   - Store delta at block 101: `{ delta_minus: 10+fee, nonce_delta: 1 }`
   - Store snapshot at block 101: new balance

3. **ERC20 Processing (block 102)**:
   - Transaction includes `Transfer` event: `0x6e98...` → `0xABC` (50 tokens)
   - ERC20 tracker parses log
   - Update: `0xABC` token balance += 50
   - Store ERC20 delta: `{ delta_plus: 50 }`
   - Store ERC20 snapshot: new balance

4. **Query**:
   ```bash
   statectl balances 0xABC 100 105
   ```
   - Find anchor snapshot at block 100: 1000 ETH
   - Load deltas [101, 105]
   - Fill forward: 1000 → 990 → 990 → 1040 → 1040 → 1020
   - Return dense list with coverage metadata

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

// Query balances with fill-forward
let balances = store.get_balances_in_range_with_metadata(addr, 100, 105)?;
for (block, balance) in balances.data {
    println!("Block {}: {} wei", block, balance);
}
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
- Delta and snapshot storage
- Fill-forward query correctness
- Coverage clamping
- ERC20 key encoding/decoding
- ERC20 tracker behavior (mint/burn/transfer/revert)

## Running on Anvil (Local Testnet)

1. Start Anvil:
   ```bash
   anvil
   ```

2. Run the watcher:
   ```bash
   cargo run --bin watcher -- \
     --rpc-url http://127.0.0.1:8545 \
     --watchlist watchlist.txt \
     --tokens tokens.txt
   ```

3. Make some transactions:
   ```bash
   # Send ETH
   cast send 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 --value 1ether -r http://127.0.0.1:8545

   # Deploy ERC20 and transfer
   # (use your ERC20 contract deployment script)
   ```

4. Query balances:
   ```bash
   cargo run --bin statectl -- balances 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 0 10
   cargo run --bin statectl -- erc20-balances <token> <owner> 0 10
   ```

## Dependencies

- `alloy-primitives`: Ethereum types (Address, B256, U256)
- `rocksdb`: Persistent key-value database
- `postcard`: Binary serialization for structs
- `anyhow`: Error handling
- `clap`: CLI argument parsing
- `serde`: Serialization framework
- `hex`: Hex string parsing
- `reqwest`: HTTP client for JSON-RPC
- `tokio`: Async runtime
- `tracing`: Structured logging

## Performance Characteristics

- **Storage**: O(changes) not O(blocks) - only stores when balances change
- **Queries**: O(log n) for anchor lookup + O(changes in range) for deltas
- **RPC Calls**: Minimized via caching and batching
- **Memory**: Small cache, per-block accumulator cleared after persistence

## Future Extensions

The tracker system is designed to be extensible. Future trackers could include:

- **Uniswap Tracker**: Track LP positions, swaps, fees
- **Aave Tracker**: Track lending/borrowing positions
- **Compound Tracker**: Track cToken balances
- **Storage Tracker**: Track contract storage changes for specific protocols

Each tracker implements the `Tracker` trait and receives `TrackerContext` with store, RPC, watched addresses/tokens, and block number.

## License

This project is part of a larger Ethereum transaction simulation system.
