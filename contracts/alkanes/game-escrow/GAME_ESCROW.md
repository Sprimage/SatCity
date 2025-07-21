# GameEscrow Contract Documentation

## 1. Overview

The `GameEscrow` contract is a core component of the Sat City game infrastructure. Its primary responsibility is to securely hold all player-owned assets while they are being used within the game ecosystem. This includes fungible tokens (like `$CHIP`) and non-fungible tokens (like Orbitals).

The contract is designed with a trust-minimized architecture. While an off-chain server (the "Operator") is required to authorize withdrawals, the contract's logic is transparent and verifiable on-chain. A DAO has administrative control over the contract's key parameters, ensuring community oversight.

## 2. Core Concepts

### Roles

-   **Player:** Any address that deposits assets into the contract.
-   **Operator:** An off-chain server responsible for validating withdrawal requests and signing authorization messages. The operator key is a standard secp256k1 public key.
-   **DAO:** A designated address (e.g., a multisig wallet or another contract) that has administrative privileges over the escrow contract.

### Internal Ledger

The contract maintains an on-chain ledger to track all deposited assets.
-   **Fungible Tokens (FTs):** Balances are stored in a mapping: `(player_address, token_id) -> balance`.
-   **Non-Fungible Tokens (NFTs):** Ownership is stored in a mapping: `(nft_id) -> owner_address`.

### Signature-Based Withdrawals

To withdraw assets, a player must obtain a signature from the Operator. This signature authorizes a specific withdrawal (recipient, token, amount, and nonce). This design prevents players from being able" to withdraw assets directly, which is a key requirement for the game's economic model, while still ensuring that the off-chain server cannot steal funds, as it can only authorize transfers *to the asset's rightful owner*.

## 3. Function Reference

The contract exposes several functions, which are dispatched via opcodes.

---

### `Initialize`

-   **Opcode:** `0`
-   **Description:** Initializes the contract. This function can only be called once. It sets the initial DAO and Operator addresses.
-   **Parameters:**
    -   `dao_address: AlkaneId`: The address for the DAO.
    -   `operator_address: AlkaneId`: The public key of the off-chain operator.
-   **Logic:**
    1.  Checks if the contract has already been initialized. If so, it reverts.
    2.  Stores the `dao_address` and `operator_address` in contract storage.
    3.  Sets an `initialized` flag to prevent re-initialization.

---

### `Deposit`

-   **Opcode:** `1`
-   **Description:** Allows a player to deposit assets into the escrow. The assets to be deposited are taken from the `incoming_alkanes` of the transaction context.
-   **Parameters:** None.
-   **Logic:**
    1.  Checks if the contract is paused. If so, it reverts.
    2.  Iterates through all tokens sent to the contract in the transaction.
    3.  For each token, it checks if the token's contract address is on the allowlist.
    4.  If the token is allowed, it updates the internal ledger:
        -   If the token value is `1`, it's treated as an NFT, and the sender is recorded as the owner.
        -   If the token value is greater than `1`, it's treated as a fungible token, and the sender's balance for that token is increased.

---

### `Withdraw`

-   **Opcode:** `2`
-   **Description:** Withdraws assets from the escrow to a specified recipient. This action must be authorized by a valid signature from the current Operator.
-   **Parameters:**
    -   `recipient: AlkaneId`: The address to receive the assets.
    -   `token: AlkaneId`: The ID of the token (FT contract or NFT ID) to withdraw.
    -   `amount: u128`: The amount of the token to withdraw.
    -   `nonce: u128`: A sequential, per-player number to prevent replay attacks.
    -   `signature: Vec<u8>`: The ECDSA signature from the Operator.
-   **Logic:**
    1.  Checks if the contract is paused. If so, it reverts.
    2.  **Signature Verification:**
        -   Constructs a unique message hash from the `recipient`, `token`, `amount`, and `nonce`.
        -   Recovers the public key from the `signature` and the message hash.
        -   Compares the recovered public key to the stored `operator_address`. If they do not match, the transaction reverts.
    3.  **Nonce Verification:**
        -   Retrieves the last used nonce for the `recipient`.
        -   Checks that the provided `nonce` is exactly one greater than the stored nonce. If not, the transaction reverts.
    4.  **Balance & Ownership Check:**
        -   If `amount` is `1` (NFT), it verifies that the `recipient` is the recorded owner of the `token`.
        -   If `amount` is greater than `1` (FT), it verifies that the `recipient` has a sufficient balance of the `token`.
    5.  **State Update:**
        -   Updates the internal ledger (decrements FT balance or removes NFT ownership).
        -   Increments the `recipient`'s nonce in storage.
    6.  **Asset Transfer:** Returns a `CallResponse` that transfers the assets to the `recipient`.

---

### `AddTokenToAllowlist`

-   **Opcode:** `3`
-   **Access:** DAO Only
-   **Description:** Adds a token contract to the list of assets that can be deposited.
-   **Parameters:**
    -   `token: AlkaneId`: The contract address of the token to allow.
-   **Logic:**
    1.  Verifies that the caller is the DAO.
    2.  Adds the `token` to the allowlist mapping in storage.

---

### `RemoveTokenFromAllowlist`

-   **Opcode:** `4`
-   **Access:** DAO Only
-   **Description:** Removes a token contract from the deposit allowlist.
-   **Parameters:**
    -   `token: AlkaneId`: The contract address of the token to disallow.
-   **Logic:**
    1.  Verifies that the caller is the DAO.
    2.  Removes the `token` from the allowlist mapping in storage.

---

### `SetOperator`

-   **Opcode:** `5`
-   **Access:** DAO Only
-   **Description:** Updates the Operator address. This is used to rotate the off-chain signing key.
-   **Parameters:**
    -   `new_operator: AlkaneId`: The public key of the new operator.
-   **Logic:**
    1.  Verifies that the caller is the DAO.
    2.  Overwrites the `operator_address` in storage with the `new_operator` address.

---

### `SetPaused`

-   **Opcode:** `6`
-   **Access:** DAO Only
-   **Description:** Pauses or unpauses the `deposit` and `withdraw` functions. This is an emergency security feature.
-   **Parameters:**
    -   `paused: bool`: `true` to pause the contract, `false` to unpause.
-   **Logic:**
    1.  Verifies that the caller is the DAO.
    2.  Updates the `paused` flag in storage.