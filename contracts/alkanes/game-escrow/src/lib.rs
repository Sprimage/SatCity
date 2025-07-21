//! # Sat City Game Escrow Contract
//!
//! ## Chadson's Journal - Entry 13
//!
//! **Objective:** Implement the final DAO functions: setting the operator and pausing the contract.
//!
//! **Systematic Approach:**
//! 1.  **Pause Functionality:**
//!     -   I've added a `paused_pointer` to store the contract's paused state as a `u8`.
//!     -   A `SetPaused` message allows the DAO to toggle this state.
//!     -   The `set_paused` function, guarded by `_only_dao`, writes the new state to storage.
//!     -   Crucially, I've added `if self.is_paused()` checks at the beginning of both the `deposit` and `withdraw` functions. This is a critical security feature to halt contract activity in an emergency.
//! 2.  **Operator Management:**
//!     -   A `SetOperator` message allows the DAO to update the operator address.
//!     -   The `set_operator` function, also guarded by `_only_dao`, updates the `operator_address_pointer`. This allows the off-chain signing key to be rotated securely.
//! 3.  **Feature Completion:** With these changes, all the core features specified in `SATCITY.md` (deposit, withdrawal with signature, and DAO controls) are now implemented in the contract.
//!
//! **Next Steps:**
//! - The final and most critical step is to write a comprehensive test suite to validate every function and security mechanism implemented.

use alkanes_macros::MessageDispatch;
use alkanes_runtime::{declare_alkane, runtime::AlkaneResponder, storage::StoragePointer};
use alkanes_support::{
    alkane_transfer::AlkaneTransfer,
    context::Context,
    id::AlkaneId,
    parcel::AlkaneTransferParcel,
    response::CallResponse,
};
use anyhow::{anyhow, Result};
use bitcoin::hashes::{sha256, Hash, HashEngine};
use metashrew_support::index_pointer::KeyValuePointer;
use secp256k1::{ecdsa::RecoverableSignature, Message, PublicKey, Secp256k1};
use std::sync::Arc;

// --- Storage Pointers ---

/// Points to the stored DAO address.
fn dao_address_pointer() -> StoragePointer {
    StoragePointer::from_keyword("/dao")
}

/// Points to the stored Operator address.
fn operator_address_pointer() -> StoragePointer {
    StoragePointer::from_keyword("/operator")
}

/// Points to the initialization flag.
fn initialized_pointer() -> StoragePointer {
    StoragePointer::from_keyword("/initialized")
}

/// Points to the contract's paused state.
fn paused_pointer() -> StoragePointer {
    StoragePointer::from_keyword("/paused")
}

/// Points to the allowlist of token contracts.
/// Key: `AlkaneId` of the token contract. Value: `1u8` if allowed.
fn allowlist_pointer(token_id: &AlkaneId) -> StoragePointer {
    StoragePointer::from_keywords(&["/allowlist", &token_id.to_string()])
}

/// Points to a player's balance for a specific fungible token.
/// Key: `(player_address, token_id)`. Value: `u128` balance.
fn balance_pointer(player: &AlkaneId, token: &AlkaneId) -> StoragePointer {
    StoragePointer::from_keywords(&["/balances", &player.to_string(), &token.to_string()])
}

/// Points to the owner of a specific Orbital (NFT).
/// Key: `AlkaneId` of the Orbital. Value: `AlkaneId` of the owner.
fn orbital_owner_pointer(orbital_id: &AlkaneId) -> StoragePointer {
    StoragePointer::from_keywords(&["/orbitals", &orbital_id.to_string()])
}

/// Points to the last used nonce for a player to prevent replay attacks.
/// Key: `player_address`. Value: `u128` nonce.
fn nonce_pointer(player: &AlkaneId) -> StoragePointer {
    StoragePointer::from_keywords(&["/nonces", &player.to_string()])
}

/// Creates a hash of the withdrawal message parameters.
fn message_hash(recipient: &AlkaneId, token: &AlkaneId, amount: u128, nonce: u128) -> Message {
    let mut engine = sha256::Hash::engine();
    engine.input(&recipient.to_bytes());
    engine.input(&token.to_bytes());
    engine.input(&amount.to_le_bytes());
    engine.input(&nonce.to_le_bytes());
    Message::from_slice(&sha256::Hash::from_engine(engine).into_inner()).unwrap()
}

/// # GameEscrow Contract
///
/// Custodies all player-owned assets (fungible tokens and NFTs) while they are in-game.
#[derive(Default)]
pub struct GameEscrow(());

impl AlkaneResponder for GameEscrow {}

/// Defines the messages the `GameEscrow` contract can receive.
#[derive(MessageDispatch)]
pub enum GameEscrowMessage {
    /// Initializes the contract with the DAO and Operator addresses.
    #[opcode(0)]
    Initialize {
        dao_address: AlkaneId,
        operator_address: AlkaneId,
    },
    /// Deposits assets into the escrow.
    #[opcode(1)]
    Deposit,
    /// Withdraws assets from the escrow, authorized by an operator signature.
    #[opcode(2)]
    Withdraw {
        recipient: AlkaneId,
        token: AlkaneId,
        amount: u128,
        nonce: u128,
        signature: Vec<u8>,
    },
    /// (DAO only) Adds a token contract to the deposit allowlist.
    #[opcode(3)]
    AddTokenToAllowlist { token: AlkaneId },
    /// (DAO only) Removes a token contract from the deposit allowlist.
    #[opcode(4)]
    RemoveTokenFromAllowlist { token: AlkaneId },
    /// (DAO only) Sets a new operator address.
    #[opcode(5)]
    SetOperator { new_operator: AlkaneId },
    /// (DAO only) Pauses or unpauses the contract.
    #[opcode(6)]
    SetPaused { paused: bool },
}

impl GameEscrow {
    /// Helper to check if the caller is the DAO.
    fn _only_dao(&self) -> Result<()> {
        let context = self.context()?;
        let dao_address_bytes = dao_address_pointer()
            .get()
            .ok_or_else(|| anyhow!("DAO address not set"))?;
        let dao_address = AlkaneId::from_bytes(&dao_address_bytes)?;
        if context.sender != dao_address {
            return Err(anyhow!("Caller is not the DAO"));
        }
        Ok(())
    }

    /// Helper to check if the contract has been initialized.
    fn is_initialized(&self) -> bool {
        initialized_pointer().get_value::<u8>() == 1
    }

    /// Helper to check if the contract is paused.
    fn is_paused(&self) -> bool {
        paused_pointer().get_value::<u8>() == 1
    }

    /// Checks if a token is on the allowlist.
    fn is_token_allowed(&self, token_id: &AlkaneId) -> bool {
        allowlist_pointer(token_id).get_value::<u8>() == 1
    }

    /// Initializes the contract. Can only be called once.
    fn initialize(
        &self,
        dao_address: AlkaneId,
        operator_address: AlkaneId,
    ) -> Result<CallResponse> {
        if self.is_initialized() {
            return Err(anyhow!("Contract already initialized"));
        }

        // Store the addresses
        dao_address_pointer().set(Arc::new(dao_address.to_bytes()));
        operator_address_pointer().set(Arc::new(operator_address.to_bytes()));

        // Set the initialized flag
        initialized_pointer().set_value::<u8>(1);

        Ok(CallResponse::default())
    }

    /// Deposits assets into the escrow.
    fn deposit(&self) -> Result<CallResponse> {
        if self.is_paused() {
            return Err(anyhow!("Contract is paused"));
        }
        let context = self.context()?;
        if !self.is_initialized() {
            return Err(anyhow!("Contract not initialized"));
        }

        let sender = context.sender.clone();

        for transfer in context.incoming_alkanes.0.iter() {
            // Ensure the token is on the allowlist
            if !self.is_token_allowed(&transfer.id) {
                continue; // Ignore non-allowlisted tokens
            }

            // Simplified logic: Assume value of 1 is an NFT (Orbital), > 1 is FT ($CHIP)
            if transfer.value == 1 {
                // Treat as NFT
                orbital_owner_pointer(&transfer.id).set(Arc::new(sender.to_bytes()));
            } else {
                // Treat as Fungible Token
                let balance_ptr = balance_pointer(&sender, &transfer.id);
                let current_balance = balance_ptr.get_value::<u128>();
                let new_balance = current_balance.saturating_add(transfer.value);
                balance_ptr.set_value(&new_balance);
            }
        }

        Ok(CallResponse::default())
    }

    /// Withdraws assets from the escrow.
    fn withdraw(
        &self,
        recipient: AlkaneId,
        token: AlkaneId,
        amount: u128,
        nonce: u128,
        signature: Vec<u8>,
    ) -> Result<CallResponse> {
        if self.is_paused() {
            return Err(anyhow!("Contract is paused"));
        }
        if !self.is_initialized() {
            return Err(anyhow!("Contract not initialized"));
        }

        // --- Signature Verification ---
        let operator_address_bytes = operator_address_pointer()
            .get()
            .ok_or_else(|| anyhow!("Operator address not set"))?;
        let operator_pubkey = PublicKey::from_slice(&operator_address_bytes)?;

        let signature = RecoverableSignature::from_compact(&signature, secp256k1::ecdsa::RecoveryId::from_i32(0)?)?;

        let hash = message_hash(&recipient, &token, amount, nonce);
        let recovered_pubkey = Secp256k1::new().recover_ecdsa(&hash, &signature)?;

        if recovered_pubkey != operator_pubkey {
            return Err(anyhow!("Invalid signature"));
        }

        // --- Nonce Verification ---
        let nonce_ptr = nonce_pointer(&recipient);
        let current_nonce = nonce_ptr.get_value::<u128>();
        if nonce != current_nonce + 1 {
            return Err(anyhow!("Invalid nonce"));
        }

        // --- Asset & Balance Verification ---
        if amount == 1 { // Assume NFT
            let owner_ptr = orbital_owner_pointer(&token);
            let owner_bytes = owner_ptr.get().ok_or_else(|| anyhow!("NFT not found in escrow"))?;
            let owner_id = AlkaneId::from_bytes(&owner_bytes)?;
            if owner_id != recipient {
                return Err(anyhow!("Recipient is not the owner of this NFT"));
            }
            owner_ptr.clear(); // Remove ownership record
        } else { // Assume FT
            let balance_ptr = balance_pointer(&recipient, &token);
            let current_balance = balance_ptr.get_value::<u128>();
            let new_balance = current_balance.checked_sub(amount).ok_or_else(|| anyhow!("Insufficient balance"))?;
            balance_ptr.set_value(&new_balance);
        }

        // Update nonce after successful verification and balance check
        nonce_ptr.set_value(&(nonce));

        // Create response to transfer the asset
        let response = CallResponse {
            alkanes: AlkaneTransferParcel(vec![AlkaneTransfer {
                id: token,
                value: amount,
            }]),
            ..Default::default()
        };

        Ok(response)
    }

    /// (DAO only) Adds a token to the allowlist.
    fn add_token_to_allowlist(&self, token: AlkaneId) -> Result<CallResponse> {
        self._only_dao()?;
        allowlist_pointer(&token).set_value(&1u8);
        Ok(CallResponse::default())
    }

    /// (DAO only) Removes a token from the allowlist.
    fn remove_token_from_allowlist(&self, token: AlkaneId) -> Result<CallResponse> {
        self._only_dao()?;
        allowlist_pointer(&token).set_value(&0u8);
        Ok(CallResponse::default())
    }

    /// (DAO only) Sets a new operator address.
    fn set_operator(&self, new_operator: AlkaneId) -> Result<CallResponse> {
        self._only_dao()?;
        operator_address_pointer().set(Arc::new(new_operator.to_bytes()));
        Ok(CallResponse::default())
    }

    /// (DAO only) Pauses or unpauses the contract.
    fn set_paused(&self, paused: bool) -> Result<CallResponse> {
        self._only_dao()?;
        paused_pointer().set_value(&(if paused { 1u8 } else { 0u8 }));
        Ok(CallResponse::default())
    }
}

// Declare the contract's entry points for the Alkanes VM.
declare_alkane! {
    impl AlkaneResponder for GameEscrow {
        type Message = GameEscrowMessage;
    }
}