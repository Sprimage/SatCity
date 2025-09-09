# Flow Logic – Deposits, Gameplay, and Withdrawals

The end-to-end flow of assets and state in Sat City is as follows (MVP version):
   - **Player Registration (L1)**: User calls a `register` opcode on the GameEscrow contract which creates a PlayerID  Orbital. The PlayerID's AlkaneId is recorded to be used to identify the user in subsequent calls

    - **Deposit (L1→L2)**: The user initiates a deposit by sending assets to the GameEscrow bridge contract:
    The user constructs a Bitcoin transaction calling the Bridge’s deposit opcode, including X units of $CHIPS and/or valid Orbitals along their PlayerID Orbital in the inputs. 

    The GameEscrow bridge contract, upon execution, locks the assets. The deposit transaction will be indexed by the ALKANES indexer, so the system knows the Bridge now holds those assets.

    - **L2 Mirroring (Sequencer)**: The sequencer, watching Bitcoin mempool and blocks, sees the Bridge deposit transaction and updates the L2 state: it credits the appropriate assets to the specified PlayerID account. In practice, this means adding the asset to the PlayerID’s Merkle tree leaf (in the off-chain state). For a token, increment the balance; for an item NFT, mark it as owned by that PlayerID. The deposit is now complete from the user’s perspective – their L2 balance is updated within a block or two (once the Bitcoin tx is confirmed)

    - **Gameplay (L2 off-chain)**: Players can now use their assets in the Sat City L2 environment. They may place bets with $CHIPS, trade items, or participate in games. Each action generates transactions on L2, which the sequencer processes. State management: We maintain a Merkle tree of all PlayerIDs’ states (each leaf might contain a player’s token balances and item ownerships). When a player’s state changes (due to a move or reward), the leaf for their PlayerID is updated. The Merkle root of the state is the succinct representation we track and periodically commit to L1.

    - **Batching and Proving**: The sequencer batches a set of L2 transactions (e.g., all gameplay actions
    over some interval) and computes the new state root after applying them. It then feeds the previous state root, the batched transactions, and the resulting new root into the Cairo prover. The prover runs the Cairo program that encapsulates Sat City’s state transition logic, and produces a zk-STARK proof that “Given old state root `R_old` and transactions `T`, the new state root is `R_new`”. This proof attests to the correctness of all in-game computations (no cheating by the sequencer).

    - **Posting ZK Proof (L1)**: The sequencer now submits a Bitcoin transaction to finalize the L2 batch. This transaction calls the adaptation of Cairokane verifier contract on L1, passing in the proof data. The Bridge contract may be the one to hold the canonical state root, so the transaction could actually call a method like
    `Bridge.verify_and_update(root, proof)` which internally invokes the Cairo-VM verification. The key point is that the proof is verified on-chain. If the proof is valid, the Bridge updates its stored state root to `R_new` (for example, by writing to its ALKANES contract storage the new root hash). If invalid, the transaction would fail or be ignored by the indexer (ensuring an invalid state cannot be finalized).

    - **Withdrawal (L2→L1)**: A player who wishes to exit some assets performs the following:

     - **Initiate withdraw in L2**: The player signals to the sequencer (e.g., via a special transaction) which
        assets to withdraw. The sequencer will remove those assets from the PlayerID’s state and include this in the next state update proof.

     - **Proof and state update**: The next zk-proof will include the new state (with the asset removed from
        PlayerID) and possibly an exit list of assets to release.

     - **Withdraw on L1**: After that proof is verified on L1 (meaning the exit is finalized off-chain), the user
        submits a withdrawal transaction on Bitcoin. This calls the Bridge’s `withdraw` function.

        