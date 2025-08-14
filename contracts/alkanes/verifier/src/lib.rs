//! Sat City Verifier contract
//!
//! Responsibilities:
//! - Read a Cairo STARK proof from the Bitcoin transaction witness
//! - Verify it against the stwo Cairo AIR verifier lite
//! - If valid, update the canonical L2 state root stored in contract storage
//!
//! Payload format in witness (index 0):
//! - Bytes: "SATC" (magic)
//! - u8 version (currently 1)
//! - u8 preprocessed variant: 0 = Canonical, 1 = CanonicalWithoutPedersen
//! - u32 be: number of field elements N
//! - N elements of 32 bytes each: big-endian starknet_ff::FieldElement
//! - u32 be: length L of new_root bytes
//! - L bytes: new_root (expected 32 bytes)
//!
//! See ESSENTIAL_ALKANES_CONTRACTS_CHEATSHEET.md (Rule 27) for witness reading.

use alkanes_runtime::{
    auth::AuthenticatedResponder, declare_alkane, message::MessageDispatch, runtime::AlkaneResponder, storage::StoragePointer,
};
use alkanes_support::{context::Context, id::AlkaneId, response::CallResponse, witness::find_witness_payload};
use anyhow::{anyhow, Result};
use cairo_air_verifier_lite::verifier::verify_cairo;
use cairo_air_verifier_lite::{air::CairoProof, PreProcessedTraceVariant};
use starknet_ff::FieldElement;
use bitcoin::hashes::Hash;
use bitcoin::{Transaction, Txid};
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_decode;
use std::io::Cursor;
use std::sync::Arc;
use stwo::core::vcs::blake2_merkle::{Blake2sMerkleChannel, Blake2sMerkleHasher};

pub struct ContextHandle(());

#[cfg(test)]
impl ContextHandle {
    /// Get the current transaction bytes
    pub fn transaction(&self) -> Vec<u8> {
        // This is a placeholder implementation that would normally
        // access the transaction from the runtime context
        Vec::new()
    }
}

impl AlkaneResponder for ContextHandle {}


pub const CONTEXT: ContextHandle = ContextHandle(());

/// Extension trait for Context to add transaction_id method
trait ContextExt {
    /// Get the transaction ID from the context
    fn transaction_id(&self) -> Result<Txid>;
}

#[cfg(test)]
impl ContextExt for Context {
    fn transaction_id(&self) -> Result<Txid> {
        // Test implementation with all zeros
        Ok(Txid::from_slice(&[0; 32]).unwrap_or_else(|_| {
            // This should never happen with a valid-length slice
            panic!("Failed to create zero Txid")
        }))
    }
}

#[cfg(not(test))]
impl ContextExt for Context {
    fn transaction_id(&self) -> Result<Txid> {
        Ok(
            consensus_decode::<Transaction>(&mut std::io::Cursor::new(CONTEXT.transaction()))?
                .compute_txid(),
        )
    }
}

// Storage keys
fn initialized_pointer() -> StoragePointer {
    StoragePointer::from_keyword("/initialized")
}
fn bridge_pointer() -> StoragePointer {
    StoragePointer::from_keyword("/bridge_id")
}
fn state_root_pointer() -> StoragePointer {
    StoragePointer::from_keyword("/state_root")
}
fn last_variant_pointer() -> StoragePointer {
    StoragePointer::from_keyword("/last_preprocessed_variant")
}
// No witness storage key: read witness bytes from the current transaction

#[derive(Default)]
pub struct Verifier;

impl AlkaneResponder for Verifier {}
impl AuthenticatedResponder for Verifier {}

#[derive(MessageDispatch)]
pub enum VerifierMessage {
    // Initialize and set the authorized bridge/GameEscrow contract that can call VerifyAndUpdate
    #[opcode(0)]
    Initialize { bridge: AlkaneId },
    // Verifies proof from witness and updates `/state_root`.
    // No inputs; reads payload from tx witness per the format documented above.
    #[opcode(1)]
    VerifyAndUpdate,
    // Returns the latest state root bytes
    #[opcode(97)]
    #[returns(Vec<u8>)]
    GetStateRoot,
}

impl Verifier {
    fn is_initialized(&self) -> bool {
        initialized_pointer().get_value::<u8>() == 1
    }

    fn set_initialized(&self) {
        initialized_pointer().set_value::<u8>(1);
    }

    // Bridge setter retained for future; not used in auth for simplicity.
    fn set_bridge(&self, id: AlkaneId) {
        let mut p = bridge_pointer();
        p.set(Arc::new(id.into()));
    }

    fn set_state_root(&self, root: &[u8]) {
        state_root_pointer().set(Arc::new(root.to_vec()));
    }

    fn state_root_bytes(&self) -> Vec<u8> {
        state_root_pointer().get().as_ref().clone()
    }

    fn set_last_variant(&self, v: u8) { last_variant_pointer().set(Arc::new(vec![v])); }

    fn read_witness_payload(&self) -> Result<Vec<u8>> {
        let tx = consensus_decode::<Transaction>(&mut Cursor::new(CONTEXT.transaction()))?;
        let data: Vec<u8> = find_witness_payload(&tx, 0).unwrap_or_else(|| vec![]);
        Ok(data)
    }

    fn parse_payload(
        &self,
        mut bytes: &[u8],
    ) -> Result<(PreProcessedTraceVariant, Vec<FieldElement>, Vec<u8>)> {
        // Expect magic
        if bytes.len() < 4 {
            return Err(anyhow!("PAYLOAD_TOO_SHORT"));
        }
        let magic = &bytes[0..4];
        if magic != b"SATC" {
            return Err(anyhow!("BAD_MAGIC"));
        }
        if bytes.len() < 6 {
            return Err(anyhow!("PAYLOAD_TOO_SHORT"));
        }
        let version = bytes[4];
        if version != 1 {
            return Err(anyhow!("UNSUPPORTED_VERSION"));
        }
        let variant_byte = bytes[5];
        let preprocessed_variant = match variant_byte {
            0 => PreProcessedTraceVariant::Canonical,
            1 => PreProcessedTraceVariant::CanonicalWithoutPedersen,
            _ => return Err(anyhow!("UNKNOWN_VARIANT")),
        };
        bytes = &bytes[6..];
        if bytes.len() < 4 {
            return Err(anyhow!("PAYLOAD_TOO_SHORT"));
        }
        let n = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        bytes = &bytes[4..];
        if bytes.len() < 32 * n + 4 {
            return Err(anyhow!("PROOF_BYTES_TOO_SHORT"));
        }
        let mut felts: Vec<FieldElement> = Vec::with_capacity(n);
        for i in 0..n {
            let word = &bytes[32 * i..32 * (i + 1)];
            let arr: [u8; 32] = word.try_into().map_err(|_| anyhow!("BAD_FELT"))?;
            let fe = FieldElement::from_bytes_be(&arr).map_err(|_| anyhow!("BAD_FELT"))?;
            felts.push(fe);
        }
        bytes = &bytes[32 * n..];
        let l = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        bytes = &bytes[4..];
        if bytes.len() < l {
            return Err(anyhow!("ROOT_BYTES_TOO_SHORT"));
        }
        let root = bytes[..l].to_vec();
        Ok((preprocessed_variant, felts, root))
    }

    fn deserialize_proof(
        &self,
        felts: &[FieldElement],
    ) -> Result<CairoProof<Blake2sMerkleHasher>> {
        use stwo_cairo_serialize::CairoDeserialize;
        let mut it = felts.iter();
        let proof: CairoProof<Blake2sMerkleHasher> = CairoProof::deserialize(&mut it);
        Ok(proof)
    }

    fn initialize(&self, bridge: AlkaneId) -> Result<CallResponse> {
        self.only_owner()?;
        if self.is_initialized() {
            return Err(anyhow!("ALREADY_INITIALIZED"));
        }
        self.observe_initialization()?;
        self.set_bridge(bridge);
        self.set_initialized();
        Ok(CallResponse::default())
    }

    fn verify_and_update(&self) -> Result<CallResponse> {
        // Authorization: owner-only for MVP
        self.only_owner()?;

        let payload = self.read_witness_payload()?;
        let (variant, felts, new_root) = self.parse_payload(&payload)?;
        let proof = self.deserialize_proof(&felts)?;

        // Verify
        verify_cairo::<Blake2sMerkleChannel>(proof, variant)
            .map_err(|e| anyhow!(format!("VERIFICATION_FAILED: {e}")))?;

        // Update storage
        self.set_state_root(&new_root);
        self.set_last_variant(match variant {
            PreProcessedTraceVariant::Canonical => 0,
            PreProcessedTraceVariant::CanonicalWithoutPedersen => 1,
        });

        Ok(CallResponse::default())
    }

    fn get_state_root(&self) -> Result<CallResponse> {
        let mut resp = CallResponse::default();
        resp.data = self.state_root_bytes();
        Ok(resp)
    }
}

declare_alkane! {
    impl AlkaneResponder for Verifier { type Message = VerifierMessage; }
}


