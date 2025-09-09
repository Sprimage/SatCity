# Sat City – Game Escrow (MVP)

Minimal escrow contract that records deposits from `incoming_alkanes` and supports an admin pause flag. Withdrawals are not implemented yet.

## Overview

- Records NFTs vs FTs by amount:
  - value == 1 → NFT ownership: `/nft/<token_id>` = owner (caller) bytes
  - value > 1 → FT balance: `/ft/<caller>/<token_id>` += value
- Owner can pause/unpause deposits.

## ABI (Opcodes)

- 0: Initialize { verifier: AlkaneId }
  - Marks contract initialized. Parameter currently unused.
- 1: Deposit
  - Iterates over `incoming_alkanes` and records ownership/balances; reverts if paused.
- 6: SetPaused { paused: u128 }
  - Owner-only. Non-zero pauses; zero unpauses.

## Storage

- `/initialized` → u8
- `/paused` → u8
- `/nft/<token_id_bytes>` → owner bytes (AlkaneId)
- `/ft/<caller_bytes>/<token_id_bytes>` → u128 balance

## Build

From `contracts/`:

```bash
AR=/opt/homebrew/opt/llvm/bin/llvm-ar \
CC=/opt/homebrew/opt/llvm/bin/clang \
cargo build --target wasm32-unknown-unknown -p game-escrow --release
```

## Testing

Workspace build of `sat-city-contracts` runs `contracts/build.rs`, which:
- Builds each `alkanes/*` crate to wasm
- Generates `contracts/src/tests/std/game_escrow_build.rs` exposing `get_bytes()`

Example (sketch):

```rust
use std::game_escrow_build;
let wasm = game_escrow_build::get_bytes();
// deploy via alkanes test-utils
// call Initialize, then send incoming_alkanes and call Deposit
```

See `contracts/ESSENTIAL_ALKANES_CONTRACTS_CHEATSHEET.md` for storage/test patterns.
