use std::collections::VecDeque;
use crate::state::{AlkaneId};
use ethnum::U256;


#[derive(Clone, Debug)]
pub enum Transaction {
    #[allow(dead_code)]
    TransferChips { from: AlkaneId, to: AlkaneId, amount: U256 },
    #[allow(dead_code)]
    TransferNft { from: AlkaneId, to: AlkaneId, nft_id: U256 },
}

pub struct Mempool {
    transactions: VecDeque<Transaction>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            transactions: VecDeque::new(),
        }
    }

    pub fn add_transaction(&mut self, transaction: Transaction) {
        self.transactions.push_back(transaction);
    }

    pub fn get_transactions(&mut self, n: usize) -> Vec<Transaction> {
        self.transactions.drain(0..n.min(self.transactions.len())).collect()
    }
}