use rs_merkle::{algorithms::Sha256, Hasher, MerkleTree};
use ethnum::U256;
use std::collections::HashMap;

/// Matches the Cairo struct 1 : 1
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct AlkaneId {
    pub block: u128,
    pub tx:    u128,
}

/// Mirrors the Cairo `Player`
#[derive(Clone, Debug)]
pub struct Player {
    pub id:            AlkaneId,
    pub chips_balance: U256,
}

impl Default for Player {
    fn default() -> Self {
        Self { id: AlkaneId { block: 0, tx: 0 }, chips_balance: U256::ZERO }
    }
}

/// Mirrors the Cairo `OrbitalNFT`
#[derive(Clone, Debug)]
pub struct OrbitalNft {
    pub id:    U256,
    pub owner: AlkaneId,
}

pub struct State {
    tree:    MerkleTree<Sha256>,
    players: HashMap<AlkaneId, Player>,
    nfts:    HashMap<U256, OrbitalNft>,
}

impl State {
    /// Empty tree / maps – cheapest constructor.
    pub fn new() -> Self {
        Self { tree: MerkleTree::new(), players: HashMap::new(), nfts: HashMap::new() }
    }

    /* ---------- Mutators  ---------- */

    pub fn upsert_player(&mut self, player: Player) {
        let leaf_hash = hash_player(&player);
        self.tree.insert(leaf_hash);
        self.players.insert(player.id, player);          // overwrites if exists
    }

    pub fn upsert_nft(&mut self, nft: OrbitalNft) {
        let leaf_hash = hash_nft(&nft);
        self.tree.insert(leaf_hash);
        self.nfts.insert(nft.id, nft);
    }

    /// Finalises current batch – call once per block.
    pub fn commit(&mut self) { self.tree.commit(); }

    /* ---------- Getters  ---------- */

    pub fn player(&self, id: &AlkaneId) -> Option<&Player> { self.players.get(id) }
    pub fn nft(&self, id: &U256)       -> Option<&OrbitalNft> { self.nfts.get(id) }

    pub fn root(&self) -> Option<[u8; 32]> { self.tree.root() }

    /// Flat lists the prover expects.
    pub fn players_list(&self) -> Vec<Player>     { self.players.values().cloned().collect() }
    pub fn nfts_list(&self)    -> Vec<OrbitalNft> { self.nfts.values().cloned().collect() }
}

/* ---------- Helpers: deterministic hashing ---------- */

fn hash_player(p: &Player) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(1 + 16 + 16 + 32);      // tag + id + balance
    bytes.push(0x00);                                          // player-tag
    bytes.extend_from_slice(&p.id.block.to_le_bytes());        // little-endian per Rust docs :contentReference[oaicite:5]{index=5}
    bytes.extend_from_slice(&p.id.tx.to_le_bytes());
    bytes.extend_from_slice(&p.chips_balance.to_le_bytes());   // U256::to_le_bytes() → [u8; 32] :contentReference[oaicite:6]{index=6}
    Sha256::hash(&bytes)
}

fn hash_nft(n: &OrbitalNft) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(1 + 32 + 16 + 16);
    bytes.push(0x01);                                          // nft-tag
    bytes.extend_from_slice(&n.id.to_le_bytes());
    bytes.extend_from_slice(&n.owner.block.to_le_bytes());
    bytes.extend_from_slice(&n.owner.tx.to_le_bytes());
    Sha256::hash(&bytes)
}
