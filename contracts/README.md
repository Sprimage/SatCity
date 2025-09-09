# Sat City Contracts

This directory contains the Alkanes smart contracts for the Sat City game. It is structured as a Cargo workspace to manage multiple contracts and shared libraries.

## Workspace Structure

-   `/alkanes`: This directory contains all Alkanes smart contracts. Each contract is its own crate.
    -   `/game-escrow`: The primary escrow contract for managing in-game assets.
    -   `/verifier`: ZKP verifier using stwo-cairo
-   `/src`: This directory is part of the root crate but is not currently used for contract development.
-   `Cargo.toml`: The root manifest for the workspace. It defines the workspace members and shared dependencies, including the Alkanes framework, cryptographic libraries, and other utilities.

## Development

### Building Contracts

All contracts are built to the `wasm32-unknown-unknown` target. To build the entire workspace, run the following command from within the `contracts` directory:

```bash
# Note: The AR and CC environment variables may be required on macOS
# to specify the correct LLVM toolchain for cross-compilation.
AR=/opt/homebrew/opt/llvm/bin/llvm-ar CC=/opt/homebrew/opt/llvm/bin/clang cargo build -p verifier -p game-escrow --target wasm32-unknown-unknown
```

### Dependencies

Key dependencies are managed in the root `Cargo.toml` and inherited by the individual contract crates. This includes:
-   `alkanes-rs`: The core framework for Alkanes smart contract development.
-   `bitcoin`: Provides types and utilities for handling Bitcoin-related data.
...and others