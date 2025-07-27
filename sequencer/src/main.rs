mod rpc;
mod state;
mod mempool;
mod prover;
mod helpers;

use prover::Prover;
use state::State;

#[tokio::main]                    
async fn main() {
    let prover = Prover::new();
    let state  = State::new();
    let txs: Vec<mempool::Transaction> = Vec::new();   // empty block
    let new_root = prover.prove(&txs, &state) 
            .expect("Cairo program failed");
    println!("New root: 0x{}", hex::encode(new_root));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mempool::{Mempool, Transaction};
    use crate::state::{AlkaneId, Player, OrbitalNft, State};
    use ethnum::U256;

    fn id(block: u128, tx: u128) -> AlkaneId {
        AlkaneId { block, tx }
    }

    #[test]
    fn chips_and_nft_flow() {
        /* ---------- arrange ---------- */
        let mut state = State::new();

        // two players
        let p1 = Player { id: id(1, 1), chips_balance: U256::from(100u128) };
        let p2 = Player { id: id(1, 2), chips_balance: U256::from(50u128) };
        state.upsert_player(p1.clone());
        state.upsert_player(p2.clone());

        // one NFT owned by p1
        let nft = OrbitalNft { id: U256::from(42u128), owner: p1.id };
        state.upsert_nft(nft.clone());

        // seal the pre-state Merkle root
        state.commit();
        let old_root = state.root().expect("root must exist");

        /* ---------- build block ---------- */
        let mut mempool = Mempool::new();
        mempool.add_transaction(Transaction::TransferChips {
            from: p1.id,
            to:   p2.id,
            amount: 10u128.into(),
        });
        mempool.add_transaction(Transaction::TransferNft {
            from: p1.id,
            to:   p2.id,
            nft_id: nft.id,
        });

        /* ---------- prove ---------- */
        let prover = Prover::new();
        let txs    = mempool.get_transactions(usize::MAX);
        let new_root = prover.prove(&txs, &state)
                    .expect("Cairo program failed");

        /* ---------- assert ---------- */
        assert_ne!(old_root, new_root);        // roots must differ :contentReference[oaicite:6]{index=6}
    }
}
