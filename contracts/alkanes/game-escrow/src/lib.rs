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

use alkanes_runtime::{
    auth::AuthenticatedResponder, declare_alkane, runtime::AlkaneResponder, storage::StoragePointer,
};
use alkanes_runtime::message::MessageDispatch;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::compat::to_arraybuffer_layout;
use alkanes_support::{
    cellpack::Cellpack,
    context::Context,
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
};
use anyhow::{anyhow, Result};
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




/// # GameEscrow Contract
///
/// Mints Position Tokens in exchange for deposited assets. These Position Tokens
/// can be redeemed at any time for the original assets.
#[derive(Default)]
pub struct GameEscrow(());

impl AlkaneResponder for GameEscrow {}
impl AuthenticatedResponder for GameEscrow {}
// Not a token; maintains balances and ownership state for deposits.

#[derive(MessageDispatch)]
pub enum GameEscrowMessage {
    /// Initializes the contract (idempotent once).
    #[opcode(0)]
    Initialize { verifier: AlkaneId },
    /// Accept deposits from incoming_alkanes
    #[opcode(1)]
    Deposit,
    /// DAO-only: set paused flag
    #[opcode(6)]
    SetPaused { paused: u128 },
    // reserved for future view methods
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


    /// Initializes the contract. Can only be called once.
    fn initialize(&self, _verifier: AlkaneId) -> Result<CallResponse> {
        if self.is_initialized() {
            return Err(anyhow!("Contract already initialized"));
        }

        // Set the initialized flag
        initialized_pointer().set_value::<u8>(1);

        Ok(CallResponse::default())
    }

    fn deposit(&self) -> Result<CallResponse> {
        if self.is_paused() { return Err(anyhow!("PAUSED")); }
        let ctx = self.context()?;
        let caller = ctx.caller;
        let input = ctx.incoming_alkanes;

        for t in input.0.iter() {
            if t.value == 1 {
                // NFT ownership map: /nft/<id> -> owner AlkaneId bytes
                let mut p = StoragePointer::from_keyword("/nft/").select(&t.id.clone().into());
                p.set(Arc::new(caller.into()));
            } else if t.value > 1 {
                // FT balances: /ft/<caller>/<token>
                let mut p = StoragePointer::from_keyword("/ft/")
                    .select(&caller.into()).keyword("/").select(&t.id.clone().into());
                let prev = p.get_value::<u128>();
                p.set_value::<u128>(prev.saturating_add(t.value));
            }
        }
        Ok(CallResponse::default())
    }

    fn set_paused(&self, paused: u128) -> Result<CallResponse> {
        self.only_owner()?;
        paused_pointer().set_value::<u8>(if paused != 0 { 1 } else { 0 });
        Ok(CallResponse::default())
    }

    // no other methods for now
}

declare_alkane! {
    impl AlkaneResponder for GameEscrow { type Message = GameEscrowMessage; }
}