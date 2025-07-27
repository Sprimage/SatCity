#[derive(Copy, Drop, PartialEq, Serde)]
struct AlkaneId {
    block: u128,
    tx:    u128,
}

#[derive(Copy, Drop, PartialEq, Serde)]
struct Player {
    id:             AlkaneId,
    chips_balance:  u256,
}

#[derive(Copy, Drop, PartialEq, Serde)]
struct OrbitalNFT {
    id:    u256,
    owner: AlkaneId,
}

#[derive(Drop)]
struct GameState {
    players: Array<Player>,
    nfts:    Array<OrbitalNFT>,
}


fn find_player(players: @Array<Player>, pid: @AlkaneId) -> Option<Player> {
    let mut i = 0;
    loop {
        if i == players.len() {
            return Option::None(());
        }
        let p = *players.at(i);
        if p.id == *pid { return Option::Some(p); }
        i += 1;
    }
}

fn find_nft(nfts: @Array<OrbitalNFT>, nid: @u256) -> Option<OrbitalNFT> {
    let mut i = 0;
    loop {
        if i == nfts.len() {
            return Option::None(());
        }
        let n = *nfts.at(i);
        if n.id == *nid { return Option::Some(n); }
        i += 1;
    }
}

fn update_player_balance(ref ps: Array<Player>, pid: AlkaneId, new_bal: u256) {
    let mut tmp = ArrayTrait::new();
    let mut found = false;
    let mut i = 0;
    loop {
        if i == ps.len() { break; }
        let mut cur = *ps.at(i);
        if cur.id == pid {
            cur.chips_balance = new_bal;
            found = true;
        }
        tmp.append(cur);
        i += 1;
    };
    ps = tmp;
    assert(found, 'player not found');
}

fn update_nft_owner(ref nfts: Array<OrbitalNFT>, nid: u256, new_owner: AlkaneId) {
    let mut tmp = ArrayTrait::new();
    let mut found = false;
    let mut i = 0;
    loop {
        if i == nfts.len() { break; }
        let mut cur = *nfts.at(i);
        if cur.id == nid {
            cur.owner = new_owner;
            found = true;
        }
        tmp.append(cur);
        i += 1;
    };
    nfts = tmp;
    assert(found, 'nft not found');
}


#[derive(Drop, Copy, Serde)]
enum Transaction {
    TransferChips:(AlkaneId, AlkaneId, u256),
    TransferNFT  :(AlkaneId, AlkaneId, u256),
}

fn apply_tx(ref st: GameState, tx: Transaction) {
    match tx {
        Transaction::TransferChips(data) => {
            let (from, to, amt) = data;
            let from_p = find_player(@st.players, @from).expect('from missing');
            let to_p   = find_player(@st.players, @to  ).expect('to missing');
            assert!(from_p.chips_balance >= amt, "insufficient");

            update_player_balance(ref st.players, from, from_p.chips_balance - amt);
            update_player_balance(ref st.players, to,   to_p.chips_balance   + amt);
        },
        Transaction::TransferNFT(data) => {
            let (from, to, nid) = data;
            let nft   = find_nft(@st.nfts, @nid).expect('nft missing');
            assert!(nft.owner == from, "not owner");
            update_nft_owner(ref st.nfts, nid, to);
        },
    }
}

fn process(mut st: GameState, txs: Array<Transaction>) -> GameState {
    let mut i = 0;
    loop {
        if i == txs.len() { break; }
        apply_tx(ref st, *txs.at(i));
        i += 1;
    };
    st
}


//
// ABI **exactly** matches the three `FuncArg::Array` values
// your Rust prover now supplies:
//   1. players array (ptr,len)
//   2. nfts    array (ptr,len)
//   3. txs     array (ptr,len)
// Return value: two felts (low, high) – 256-bit root placeholder.
//

fn main(raw : Array<felt252>) -> Array<felt252> {

    let mut span = raw.span();
    
   // Deserialize the three length-prefixed arrays in order.
    let players: Array<Player> = Serde::deserialize(ref span).expect('bad players array'); // pops players
    let nfts:    Array<OrbitalNFT>  = Serde::deserialize(ref span).expect('bad nfts array'); // pops nfts
    let txs:     Array<Transaction> = Serde::deserialize(ref span).expect('bad txs array'); // pops txs

    let state      = GameState { players: players, nfts: nfts };
    let new_state  = process(state, txs);

    // Cast len() (u32) → u128 so the return type matches
    let root_low : u128 = new_state.players.len().into();
    let root_high: u128 = new_state.nfts.len().into();

    let mut out: Array<felt252> = array![];
    root_low.serialize(ref out); 
    root_high.serialize(ref out);
    out
}
