use crate::state::{GameState, Player, AlkaneId};

// This function checks if a player has enough chips to cover a certain amount.
// It will panic if the balance is insufficient.
fn has_sufficient_balance(
    state: @GameState,
    player_id: @AlkaneId,
    amount: u256
) {
    let player = get_player(state, player_id);
    assert(player.chips_balance >= amount, 'Insufficient balance');
}

// A helper function to find a player by their ID.
// This is a naive implementation and will be optimized later.
fn get_player(state: @GameState, player_id: @AlkaneId) -> @Player {
    let mut i = 0;
    loop {
        if i == state.players.len() {
            // This should not happen in a valid state.
            panic('Player not found');
        }
        let player = state.players.at(i);
        if player.id.block == player_id.block && player.id.tx == player_id.tx {
            break player;
        }
        i += 1;
    }
}