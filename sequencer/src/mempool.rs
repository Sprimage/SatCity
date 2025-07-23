use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub enum Transaction {
    #[allow(dead_code)]
    TransferChips { from: [u8; 32], to: [u8; 32], amount: u128 },
    #[allow(dead_code)]
    TransferNft { from: [u8; 32], to: [u8; 32], nft_id: u128 },
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