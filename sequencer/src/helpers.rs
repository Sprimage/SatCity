use cairo_vm::{Felt252};
use ethnum::U256;
use crate::state::{Player, OrbitalNft, AlkaneId};
use crate::mempool::{Transaction};
use cairo_vm::types::relocatable::MaybeRelocatable;

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

pub fn as_felt(value: &MaybeRelocatable) -> Felt252 {
    match value {
        MaybeRelocatable::Int(f) => *f,
        _ => panic!("expected an integer felt, got relocatable"),
    }
}

fn next_felt<'a, I>(it: &mut I) -> Felt252
where
    I: Iterator<Item = &'a MaybeRelocatable>,
{
    as_felt(it.next().expect("not enough data"))
}


pub fn decode_players<'a, I>(it: &mut I) -> Vec<Player>
where
    I: Iterator<Item = &'a MaybeRelocatable>,
{
    let len: usize = next_felt(it).to_biguint().try_into().unwrap();

    (0..len)
        .map(|_| {
            Player {
                id: AlkaneId {
                    block: next_felt(it).to_biguint().try_into().unwrap(),
                    tx:    next_felt(it).to_biguint().try_into().unwrap(),
                },
                chips_balance: {
                    let lo = next_felt(it).to_biguint();
                    let hi = next_felt(it).to_biguint();
                    U256::from_words(hi.try_into().unwrap(), lo.try_into().unwrap())
                },
            }
        })
        .collect()
}

pub fn decode_nfts<'a, I>(it: &mut I) -> Vec<OrbitalNft>
where
    I: Iterator<Item = &'a MaybeRelocatable>,
{
    // first felt is the array length
    let len: usize = next_felt(it).to_biguint().try_into().unwrap();
    let mut nfts = Vec::with_capacity(len);

    for _ in 0..len {
        // u256 -> two felts (little-endian: low first, then high)
        let lo = next_felt(it).to_biguint();
        let hi = next_felt(it).to_biguint();
        let id = U256::from_words(
            hi.try_into().unwrap(),   // high 128 bits
            lo.try_into().unwrap(),   // low  128 bits
        );

        // AlkaneId -> two u128 felts
        let owner_block: u128 = next_felt(it).to_biguint().try_into().unwrap();
        let owner_tx:    u128 = next_felt(it).to_biguint().try_into().unwrap();
        let owner = AlkaneId { block: owner_block, tx: owner_tx };

        nfts.push(OrbitalNft { id, owner });
    }

    nfts
}