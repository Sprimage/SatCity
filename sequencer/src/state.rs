use rs_merkle::{MerkleTree, Hasher, algorithms::Sha256};
use std::collections::HashMap;

#[derive(Clone)]
pub struct Player {
    #[allow(dead_code)]
    pub id: [u8; 32],
    #[allow(dead_code)]
    pub chips_balance: u128,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            id: [0; 32],
            chips_balance: 0,
        }
    }
}

pub struct State {
    #[allow(dead_code)]
    tree: MerkleTree<Sha256>,
    players: HashMap<[u8; 32], Player>,
}

impl State {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            tree: MerkleTree::new(),
            players: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn update_player(&mut self, player: Player) {
        let mut leaf_data = Vec::new();
        leaf_data.extend_from_slice(&player.id);
        leaf_data.extend_from_slice(&player.chips_balance.to_le_bytes());
        let leaf_hash = Sha256::hash(&leaf_data);
        self.tree.insert(leaf_hash);
        self.players.insert(player.id, player);
    }

    #[allow(dead_code)]
    pub fn get_player(&self, id: &[u8; 32]) -> Option<&Player> {
        self.players.get(id)
    }

    #[allow(dead_code)]
    pub fn get_root(&self) -> Option<[u8; 32]> {
        self.tree.root()
    }
}