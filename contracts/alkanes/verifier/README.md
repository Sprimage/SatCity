# Sat City – Verifier

Verifies Cairo STARK proofs (via `stwo` Cairo AIR verifier lite) from the Bitcoin transaction witness and updates the canonical L2 state root.

## Overview

- Reads witness payload (index 0) from the current transaction.
- Parses a compact format, deserializes a Cairo proof, and verifies it.
- On success, updates `/state_root` and `/last_preprocessed_variant` in storage.
- Auth: currently owner-only for both Initialize and VerifyAndUpdate.

## ABI (Opcodes)

- 0: Initialize { bridge: AlkaneId }
  - Owner-only; sets initialized flag and stores the (optional) bridge id.
- 1: VerifyAndUpdate
  - Owner-only; reads witness, verifies proof, updates state root and variant.
- 97: GetStateRoot -> Vec<u8>
  - Returns latest `state_root` bytes.

## Witness Payload Format (index 0)

- 4 bytes magic: `"SATC"`
- u8 version: 1
- u8 preprocessed variant:
  - 0 = Canonical
  - 1 = CanonicalWithoutPedersen
- u32 (BE) N: number of field elements
- N × 32 bytes: big-endian `starknet_ff::FieldElement`
- u32 (BE) L: length of new root
- L bytes: new root (expected 32 bytes)

## Storage

- `/initialized` → u8
- `/bridge_id` → bytes (AlkaneId)
- `/state_root` → bytes
- `/last_preprocessed_variant` → u8 (0 or 1)

## Build

From `contracts/`:

```bash
AR=/opt/homebrew/opt/llvm/bin/llvm-ar \
CC=/opt/homebrew/opt/llvm/bin/clang \
cargo build --target wasm32-unknown-unknown -p verifier --release
```

## Testing

Workspace build of `sat-city-contracts` runs `contracts/build.rs`, which:
- Builds each `alkanes/*` crate to wasm
- Generates `contracts/src/tests/std/verifier_build.rs` exposing `get_bytes()`

Example (sketch):

```rust
use std::verifier_build;
let wasm = verifier_build::get_bytes();
// deploy via alkanes test-utils
// provide a transaction with a valid witness at index 0
// call VerifyAndUpdate and then GetStateRoot to assert
```

See `contracts/ESSENTIAL_ALKANES_CONTRACTS_CHEATSHEET.md` for witness/test patterns.
