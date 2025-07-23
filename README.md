# SatCity

## Overview and Architecture

Sat City is a zk-rollup “appchain” built on Bitcoin’s ALKANES protocol. It uses Bitcoin L1 for security (proof
verification and asset custody) while executing game logic off-chain. The design focuses on using
Cairo-based ZK circuits and native ALKANES assets for bridging. The overall architecture is summarized
below, highlighting L1 and L2 components and data flows:

**High-level Sat City architecture**. 
 - Players deposit ALKANES assets (e.g. $CHIPS tokens or Orbital NFTs) into an L1 GameEscrow contract. 
 - The sequencer mirrors these on L2 (Sat City) in the player’s state. 
 - Off-chain, a Cairo prover generates zk-STARK proofs of state updates. 
 - Proofs are verified on Bitcoin via an adaptation of the Cairo VM contract (Cairokane - @/reference/cairokane) on our GameEscrow contract, updating the state root. 
 - Withdrawals use the verified state root and Merkle proofs of balances to release assets back on L1.

This architecture leverages Bitcoin for finality but minimizes on-chain data to avoid block space constraints. Instead of publishing full state diffs to L1, Sat City relies on succinct proofs and a sequencer-operated data API for off-chain state visibility. Below we detail the key technical components.

## ZK Circuit With Cairo

Sat City’s zero-knowledge circuits will be reimplemented in Cairo. Cairo is a Rust-inspired language purpose-built for provable computations and STARK-based validity proofs. Cairo offers a mature ecosystem (StarkNet tooling, optimized provers) and a proven track record as the first production-grade STARK platform for general computation. Cairo programs compile to an efficient assembly designed for fast proof generation, enabling complex game logic to be proven off-chain without requiring deep cryptography expertise from developers.

### Cairo-lang and Rust VM integration: 

We will use the open-source Cairo toolchain (the official cairo-lang SDK and compiler) for circuit development. On the verification side, we leverage the Rust implementation of Cairo VM by LambdaClass (cairo-vm) . This Rust Cairo VM is faster and safer than the older Python version and is already used in production for Cairo/STARK proofs . It supports running Cairo programs and even compiling to WebAssembly for on-chain use. By integrating the Rust Cairo VM, we can execute or verify Cairo programs within a Bitcoin contract environment.

### Cairokane verifier contract: 

We will adapt the Cairokane project @/reference/cairokane  – a scaffolded Cairo VM verifier built as an ALKANES contract – to verify Sat City’s state transition proofs on L1. Cairokane packages the Cairo VM logic into a WASM-based ALKANES contract, allowing a Bitcoin transaction to perform Cairo proof verification. In practice, the off-chain prover generates a STARK proof attesting to correct L2 state updates, and this proof (or relevant trace data) is submitted to an adaptation of Cairokane in our GameEscrow contract on L1. The contract then runs a Cairo VM instance inside the Bitcoin witness (via WASM) to verify the proof. This design leverages Bitcoin’s ability to include large witness data (Cairo proofs) in transactions. The adaptation of Cairokane Verifier in our GameEscrow contract will use the Rust Cairo VM (compiled to WASM) internally for efficient proof checking. In summary, Sat City will generate STARK proofs off-chain via Cairo, and verify them on-chain via a Cairo VM program, ensuring the L2 is trustlessly validated on Bitcoin.

**Zero-knowledge proof workflow with Cairo**: 
    - Off-chain, the sequencer/prover inputs the latest L2 transactions and state diff into Cairo circuits, producing a STARK proof of the new state. 
    - This proof is submitted in a Bitcoin transaction to the adaptation of Cairokane Verifier in our GameEscrow contract (an ALKANES contract embedding the Cairo VM), which checks the proof’s validity. 
    - The GameEscrow contract then accepts the verified new state root.

## GameEscrow Architecture & Native Asset Model Overhaul

Our Sat City GameEscrow contract will serve as a bridge to use native ALKANES assets exclusively, simplifying trust assumptions and integration. All deposits and withdrawals involve fungible and non-fungible tokens defined within the ALKANES protocol (instead of external assets):

   -  **Fungible Game Token ($CHIPS)**: Sat City’s in-game currency is represented as an ALKANES fungible
      token (similar to an ERC-20 on Bitcoin ). $CHIPS may be used for bets, rewards, or trading. By
      using an ALKANES token, we can leverage the standard ALKANES token interface (balance tracking,
      supply, transfers) and existing wallet/indexer support for fungibles.

   -  **Orbital NFTs (Player IDs & Items)**: All unique game assets, including the PlayerID (the player’s
      identity) and special in-game items (collectibles, equipment, etc.), are represented as Orbitals, which
      are essentially ALKANES NFT contracts. The term “Orbital” refers to smart-contract-based NFTs on
      Bitcoin – pioneered by the Alkane Pandas collection (the first smart contract NFTs on Bitcoin ) @/reference/alkane-pandas-child @/reference/alkane-pandas-collection . Each Orbital is an ALKANES contract that can hold its own state and assets. This fits Sat City’s needs, as we can treat a PlayerID or item as an NFT with on-chain provenance.

**Player registration via Orbital NFT**: When a new player joins Sat City, they register an identity by minting a
PlayerID Orbital NFT. This is done through a Factory contract (an ALKANES contract similar to the Alkane
Pandas collection contract @/reference/alkane-pandas-collection ). The factory takes a registration call from the user and produces a new Orbital NFT representing the player. We will model this on the Alkane Pandas implementation, which proved the concept of minting NFTs via ALKANES smart contracts. The Panda contracts (collection @/reference/alkane-pandas-collection  and child contracts @/reference/alkane-pandas-child) show how to generate unique IDs, and store metadata. The PlayerID NFT is minted to the user’s Bitcoin address and serves as their permanent identity in Sat City:

  - **PlayerID structure**: The PlayerID Orbital is not a static collectible; it’s an ALKANES contract with its
    own storage and balance sheet. In ALKANES, every contract is inherently a token and can hold a balance of other tokens. We leverage this feature: the PlayerID NFT will maintain the player’s inventory, stats, and token balances in its state. In essence, the PlayerID acts as the player’s account. Other contracts (GameEscrow, etc.) refer to the PlayerID’s ID to identify the player and can update or query its state for things like inventory management or experience points. Because ALKANES allows contracts to call each other and pass assets, using the PlayerID as an identity reference is straightforward.

  - **PlayerID Orbital NFT lifecycle and usage**: 
      (1) A player calls the Factory contract to register. 
      (2) The factory mints a new Orbital NFT (PlayerID) to the player’s Bitcoin address. 
      (3) The PlayerID contract (NFT) holds the player’s state: inventory (item NFTs, token balances like $CHIPS) and stats. 
      (4) Game contracts reference the PlayerID (by its NFT ID) in function calls for identity and state updates (eg.awarding items or updating stats). 
      (5) The PlayerID, being an ALKANES contract, can itself hold other alkanes (tokens/NFTs) – for instance, the player’s $CHIPS balance or item NFTs are held in the PlayerID’s balance sheet

**Native asset deposit/withdraw flows**: With this model, only ALKANES-native tokens are bridged. A player cannot directly deposit BTC or an external asset – instead, they would deposit $CHIPS or an Orbital NFT representing some game item. This simplifies the bridge logic in GameEscrow Contract as both sides of the bridge (L1 and L2) speak the same asset language. The GameEscrow contract on Alkanes L1 holds custody of deposited tokens/NFTs and releases them on withdrawals. On L2, these deposits create corresponding balances or items in the PlayerID’s state.

For example, if a player deposits 100 $CHIPS on L1, the GameEscrow bridge will lock those tokens and the sequencer will
credit 100 $CHIPS to that player’s account on L2 (under their PlayerID). If a player deposits an “Excalibur Sword” NFT (an Orbital), the GameEscrow bridge holds the NFT on L1 and the player’s L2 inventory is updated to include that item (represented by an in-game item ID).

The GameEscrow bridge contract is aware of PlayerIDs: each deposit transaction specifies the target PlayerID (e.g., in the `context.incoming_alkanes` of an ALKANES opcode call `context` parameter). The Bridge associates any other incoming valid assets with that player’s L2 identity so the sequencer knows whom to credit. 

**Withdrawal logic**: When a player wants to withdraw assets back to Bitcoin L1, they initiate a withdrawal on
L2 (e.g., by burning the token). The L2 state update (to be proven) will mark those assets as withdrawn (removed from the PlayerID’s state). After a zk-proof confirms this state (meaning the assets are now free to leave L2), the player can submit a withdrawal request on L1.

The GameEscrow bridge contract, upon verifying the latest state root and a Merkle proof that the PlayerID had the asset
balance prior to withdrawal, will release the asset:
 - For $CHIPS, the Bridge transfers the tokens from its vault to the player.
 - For an item/NFT, the Bridge contract sends the Orbital NFT back to the player’s address.


Crucially, these exits are trustless because the Bridge only honors them if the zk-proof attests to the state and a matching Merkle proof is provided. Even the sequencer cannot withdraw someone else’s funds since any withdrawal must match the proven state balances.

 **Bridge deposit and withdrawal flows for native assets**:
    - On deposit, the user sends ALKANES assets (e.g. $CHIPS tokens or an item NFT) to the Bridge contract on Bitcoin L1, which locks them. 
    - The sequencer then mints or credits equivalent assets on L2 (player’s state). 
    - On withdrawal, the player proves (via the latest zk state) that their PlayerID holds the asset on L2, and the Bridge then unlocks/releases the asset back to the user on L1.

