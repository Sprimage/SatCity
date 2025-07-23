// Represents a unique identifier for an ALKANES contract on L1,
// as defined in the ALKANES protocol.
// It's composed of the block number and transaction index where the contract was created.
#[derive(Copy, Drop, Serde)]
pub struct AlkaneId {
    pub block: u128,
    pub tx: u128,
}

#[derive(Copy, Drop, Serde)]
pub struct Player {
    // The L1 AlkaneId of the Player's Orbital NFT.
    pub id: AlkaneId,
    pub chips_balance: u256,
    // Further fields for inventory and stats can be added here.
}

#[derive(Copy, Drop, Serde)]
pub struct OrbitalNFT {
    pub id: u256,
    // The L1 AlkaneId of the Player who owns this NFT.
    pub owner: AlkaneId,
    // Metadata for the NFT can be added here.
}

#[derive(Copy, Drop, Serde)]
pub struct Chips {
    pub amount: u256,
}

#[derive(Drop, Serde)]
pub struct GameState {
    // Using a simple array for now. This will be replaced with a Merkle tree.
    pub players: Array<Player>,
    pub nfts: Array<OrbitalNFT>,
}