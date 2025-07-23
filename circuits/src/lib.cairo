mod state;
mod rules;

use state::{GameState, Player, AlkaneId};
use rules::{balance, item};

#[derive(Drop, Copy)]
pub enum Transaction {
    TransferChips(
        AlkaneId,
        AlkaneId,
        u256,
    ),
    TransferNFT(
        AlkaneId,
        AlkaneId,
        u256,
    ),
}

// This will be the main entry point for the circuit.
// It will take the previous state, a set of transactions,
// and return the new state.
pub fn transition(
    mut state: GameState,
    transactions: Array<Transaction>
) -> GameState {
    let mut i = 0;
    loop {
        if i == transactions.len() {
            break;
        }
        let transaction = *transactions.at(i);
        apply_transaction(ref state, transaction);
        i += 1;
    };
    state
}

fn apply_transaction(ref state: GameState, transaction: Transaction) {
    match transaction {
        Transaction::TransferChips(from, to, amount) => {
            balance::has_sufficient_balance(&state, from, amount);
            // Placeholder for chip transfer logic
        },
        Transaction::TransferNFT(from, to, nft_id) => {
            item::is_owner(&state, from, nft_id);
            // Placeholder for NFT transfer logic
        },
    }
}