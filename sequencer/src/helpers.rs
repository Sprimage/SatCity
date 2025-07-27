use cairo_vm::{Felt252};
use ethnum::U256;
use crate::state::{Player, OrbitalNft};
use crate::mempool::{Transaction};


/// ---- Player & NFT flattening ----------------------------------------

fn split_u256(x: U256) -> (Felt252, Felt252) {
   let (lo, hi) = x.into_words();           
   (Felt252::from(lo), Felt252::from(hi)) 
}

pub fn encode_players(players: &[Player]) -> Vec<Felt252> {
    players
        .iter()
        .flat_map(|p| {
            let (bal_lo, bal_hi) = split_u256(p.chips_balance.into());
            vec![
                Felt252::from(p.id.block),
                Felt252::from(p.id.tx),
                bal_lo,
                bal_hi,
            ]
        })
        .collect()
}

pub fn encode_nfts(nfts: &[OrbitalNft]) -> Vec<Felt252> {
    nfts.iter()
        .flat_map(|n| {
            let (id_lo, id_hi) = split_u256(n.id.into());
            vec![
                id_lo,
                id_hi,
                Felt252::from(n.owner.block),
                Felt252::from(n.owner.tx),
            ]
        })
        .collect()
}

/// ---- Transaction flattening -----------------------------------------

pub fn encode_txs(txs: &[Transaction]) -> Vec<Felt252> {
    txs.iter()
        .flat_map(|t| match t {
            Transaction::TransferChips { from, to, amount } => {
                let (a_lo, a_hi) = split_u256((*amount).into());
                vec![
                    Felt252::from(0u8),                              // tag
                    Felt252::from(from.block),
                    Felt252::from(from.tx),
                    Felt252::from(to.block),
                    Felt252::from(to.tx),
                    a_lo,
                    a_hi,
                ]
            }
            Transaction::TransferNft { from, to, nft_id } => {
                let (id_lo, id_hi) = split_u256((*nft_id).into());
                vec![
                    Felt252::from(1u8),                              // tag
                    Felt252::from(from.block),
                    Felt252::from(from.tx),
                    Felt252::from(to.block),
                    Felt252::from(to.tx),
                    id_lo,
                    id_hi,
                ]
            }
        })
        .collect()
}
