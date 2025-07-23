mod rpc;
mod state;
mod mempool;
mod prover;

#[allow(unused_imports)]
use rpc::{RpcClient, RpcConfig};
#[allow(unused_imports)]
use serde_json::json;
#[allow(unused_imports)]
use state::{State, Player};
use mempool::{Mempool, Transaction};
use prover::Prover;

#[tokio::main]
async fn main() {
    let prover = Prover::new();
    let transactions = vec![];
    let old_root = [0; 32];
    let new_root = prover.prove(&transactions, &old_root);
    println!("New root: {:?}", new_root);
}
