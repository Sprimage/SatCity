//! # Sat City Game Escrow Contract
//!
//! ## Chadson's Journal - Entry 17
//!
//! **Objective:** Complete architectural overhaul to a "Position Token" model, removing all previous incorrect authentication patterns.
//!
//! **Systematic Approach:**
//! 1.  **Embrace Position Tokens:** This is the final, correct architecture. The contract no longer deals with external auth tokens or signatures. Instead, upon deposit, it mints a unique NFT ("Position Token") that acts as a receipt and claim check for the deposited assets.
//! 2.  **Remove Obsolete Logic:** I have removed the Operator role, all `secp256k1` signature verification, nonces, and the flawed player/DAO auth token logic. The contract is now much simpler and more aligned with Alkanes standards.
//! 3.  **New `deposit` Flow:**
//!     -   The `deposit` function now takes the assets to be deposited directly from the `incoming_alkanes`.
//!     -   It records the details of the deposit (original assets and depositor) in storage, keyed by a new, unique `position_id`.
//!     -   It then calls a separate "Position Token" contract implementation to mint a new NFT representing this position, and sends it to the depositor.
//! 4.  **New `withdraw` Flow:**
//!     -   The `withdraw` function is now parameter-less. It authenticates the caller by verifying they have sent a valid Position Token created by this contract.
//!     -   It uses the ID of the incoming Position Token to look up the original deposit.
//!     -   It returns the original assets to the caller and burns the Position Token.
//! 5.  **Factory Support:** The contract now implements `FactorySupport`, the standard Alkanes trait for contracts that create other contracts (in this case, Position Tokens).
//!
//! **Next Steps:**
//! - Create the `PositionToken` contract itself.
//! - Build the workspace.
//! - Rewrite all documentation to reflect this new, correct architecture.

use alkanes_macros::MessageDispatch;
use alkanes_runtime::{
    auth::AuthenticatedResponder,
    runtime::AlkaneResponder,
    storage::StoragePointer,
};
use alkanes_support::{
    parcel::AlkaneTransfer,
    cellpack::Cellpack,
    context::Context,
    id::AlkaneId,
    parcel::AlkaneTransferParcel,
    response::CallResponse
};
use alkanes_s
use anyhow::{anyhow, Result};
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

// --- Storage Pointers ---

/// Points to the AlkaneId of the Position Token implementation contract.
fn position_token_implementation_pointer() -> StoragePointer {
    StoragePointer::from_keyword("/position_token_impl")
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
    StoragePointer::from_keyword("/allowlist")
        .select(&token_id.to_bytes())
}

/// Points to the data of a specific deposit.
/// Key: `position_id`. Value: `(depositor, original_asset_id, amount)`.
fn deposit_info_pointer(position_id: &AlkaneId) -> StoragePointer {
    StoragePointer::from_keywords(&["/deposits", &position_id.to_string()])
}

/// # GameEscrow Contract
///
/// Mints Position Tokens in exchange for deposited assets. These Position Tokens
/// can be redeemed at any time for the original assets.
#[derive(Default)]
pub struct GameEscrow(());

impl AlkaneResponder for GameEscrow {}
impl AuthenticatedResponder for GameEscrow {}
impl FactorySupport for GameEscrow {}

/// Defines the messages the `GameEscrow` contract can receive.
#[derive(MessageDispatch)]
pub enum GameEscrowMessage {
    /// Initializes the contract, setting the position token implementation.
    #[opcode(0)]
    Initialize {
        position_token_implementation: AlkaneId,
    },
    /// Deposits assets and mints a position token.
    #[opcode(1)]
    Deposit,
    /// Withdraws assets by returning a position token.
    #[opcode(2)]
    Withdraw,
    /// (DAO only) Adds a token contract to the deposit allowlist.
    #[opcode(3)]
    AllowToken { token: AlkaneId },
    /// (DAO only) Removes a token contract from the deposit allowlist.
    #[opcode(4)]
    DisallowToken { token: AlkaneId },
    /// (DAO only) Pauses or unpauses the contract.
    #[opcode(5)]
    SetPaused { paused: bool },
}

impl GameEscrow {
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
    fn initialize(&self, position_token_implementation: AlkaneId) -> Result<CallResponse> {
        if self.is_initialized() {
            return Err(anyhow!("Contract already initialized"));
        }

        // Store the position token implementation address
        position_token_implementation_pointer().set(Arc::new(position_token_implementation.to_bytes()));

        // Deploy the DAO's auth token and send it to the caller
        let dao_auth_token_transfer = self.deploy_self_auth_token(1)?;
        let mut response = CallResponse::default();
        response.alkanes.0.push(dao_auth_token_transfer);

        // Set the initialized flag
        initialized_pointer().set_value::<u8>(1);

        Ok(response)
    }

    /// Deposits assets and mints a position token.
    fn deposit(&self) -> Result<CallResponse> {
        if self.is_paused() {
            return Err(anyhow!("Contract is paused"));
        }
        let context = self.context()?;
        if !self.is_initialized() {
            return Err(anyhow!("Contract not initialized"));
        }

        // Expecting a single type of asset to be deposited at a time for simplicity.
        let deposit = context.incoming_alkanes.0.get(0).ok_or_else(|| anyhow!("No assets deposited"))?;
        if !self.is_token_allowed(&deposit.id) {
            return Err(anyhow!("Deposited token is not on the allowlist"));
        }

        // Create a new unique ID for the position token.
        let position_id = self.create_child_id()?;

        // Store the deposit information.
        let mut deposit_data = Vec::new();
        deposit_data.extend_from_slice(&context.caller.to_bytes());
        deposit_data.extend_from_slice(&deposit.id.to_bytes());
        deposit_data.extend_from_slice(&deposit.value.to_le_bytes());
        deposit_info_pointer(&position_id).set(Arc::new(deposit_data));

        // Mint the position token NFT and send it to the depositor.
        let position_token_impl_bytes = position_token_implementation_pointer().get().ok_or_else(|| anyhow!("Position token implementation not set"))?;
        let position_token_impl = AlkaneId::from_bytes(&position_token_impl_bytes)?;
        
        let mint_cellpack = Cellpack {
            target: position_token_impl,
            inputs: vec![0, position_id.tx], // Assuming opcode 0 is mint
        };
        
        self.call(&mint_cellpack, &AlkaneTransferParcel::default(), self.fuel())
    }

    /// Withdraws assets by returning a position token.
    fn withdraw(&self) -> Result<CallResponse> {
        if self.is_paused() {
            return Err(anyhow!("Contract is paused"));
        }
        let context = self.context()?;

        // Expecting a single position token to be returned.
        let position_token = context.incoming_alkanes.0.get(0).ok_or_else(|| anyhow!("No position token provided"))?;

        // Verify the incoming token is a legitimate child of this contract.
        if !self.is_registered_child(&position_token.id) {
            return Err(anyhow!("Invalid position token provided"));
        }

        // Retrieve the original deposit information.
        let deposit_data = deposit_info_pointer(&position_token.id).get().ok_or_else(|| anyhow!("Deposit info not found for position token"))?;
        let depositor = AlkaneId::from_bytes(&deposit_data[0..32])?;
        let asset_id = AlkaneId::from_bytes(&deposit_data[32..64])?;
        let amount = u128::from_le_bytes(deposit_data[64..80].try_into().unwrap());

        // Ensure the person withdrawing is the original depositor.
        if context.caller != depositor {
            return Err(anyhow!("Caller is not the original depositor"));
        }

        // Burn the position token (by not re-issuing it).
        // Return the original assets.
        let mut response = CallResponse::default();
        response.alkanes.0.push(AlkaneTransfer {
            id: asset_id,
            value: amount,
            recipient: Some(depositor),
        });

        // Clear the deposit info from storage.
        deposit_info_pointer(&position_token.id).clear();

        Ok(response)
    }

    /// (DAO only) Adds a token to the allowlist.
    fn allow_token(&self, token: AlkaneId) -> Result<CallResponse> {
        self.only_owner()?;
        allowlist_pointer(&token).set_value(&1u8);
        Ok(CallResponse::default())
    }

    /// (DAO only) Removes a token from the allowlist.
    fn disallow_token(&self, token: AlkaneId) -> Result<CallResponse> {
        self.only_owner()?;
        allowlist_pointer(&token).clear();
        Ok(CallResponse::default())
    }

    /// (DAO only) Pauses or unpauses the contract.
    fn set_paused(&self, paused: bool) -> Result<CallResponse> {
        self.only_owner()?;
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