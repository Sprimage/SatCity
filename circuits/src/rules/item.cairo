use crate::state::{GameState, OrbitalNFT, AlkaneId};

// This function checks if a player owns a specific NFT.
// It will panic if the player is not the owner.
pub fn is_owner(
    state: &GameState,
    player_id: AlkaneId,
    nft_id: u256
) {
    let nft = get_nft(state, nft_id);
    assert(nft.owner.block == player_id.block && nft.owner.tx == player_id.tx, 'Not the owner');
}

// A helper function to find an NFT by its ID.
// This is a naive implementation and will be optimized later.
fn get_nft(state: &GameState, nft_id: u256) -> OrbitalNFT {
    let mut i = 0;
    loop {
        if i == state.nfts.len() {
            // This should not happen in a valid state.
            let mut data = array!['NFT not found'];
            panic(data);
        }
        let nft = *state.nfts.at(i);
        if nft.id == nft_id {
            break nft;
        }
        i += 1;
    }
}