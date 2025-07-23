// Represents a unique identifier for an ALKANES contract on L1,
// as defined in the ALKANES protocol.
// It's composed of the block number and transaction index where the contract was created.
#[derive(Copy, Drop, Serde)]
struct AlkaneId {
    block: u128,
    tx: u128,
}

#[derive(Copy, Drop, Serde)]
struct Player {
    // The L1 AlkaneId of the Player's Orbital NFT.
    id: AlkaneId,
    chips_balance: u256,
    // Further fields for inventory and stats can be added here.
}

#[derive(Copy, Drop, Serde)]
struct OrbitalNFT {
    id: u256,
    // The L1 AlkaneId of the Player who owns this NFT.
    owner: AlkaneId,
    // Metadata for the NFT can be added here.
}

#[derive(Copy, Drop, Serde)]
struct Chips {
    amount: u256,
}

#[derive(Drop, Serde)]
struct GameState {
    // Using a simple array for now. This will be replaced with a Merkle tree.
    players: Array<Player>,
    nfts: Array<OrbitalNFT>,
}