use crate::mempool::Transaction;

pub struct Prover;

impl Prover {
    pub fn new() -> Self {
        Self
    }

    pub fn prove(
        &self,
        _transactions: &[Transaction],
        _previous_state_root: &[u8; 32],
    ) -> [u8; 32] {
        // For now, this is a stub that just returns a dummy state root.
        [0; 32]
    }
}