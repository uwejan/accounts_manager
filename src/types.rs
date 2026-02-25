use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

/// `amount` is optional because dispute/resolve/chargeback rows
/// do not carry an amount, they reference an existing transaction by tx ID.
#[derive(Debug, Clone, Deserialize)]
pub struct TransactionRecord {
    pub r#type: TransactionType,
    pub client: u16,
    pub tx: u32,
    pub amount: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct StoredTransaction {
    pub client: u16,
    pub amount: Decimal,
    pub under_dispute: bool,
}

#[derive(Debug, Clone)]
pub struct ClientAccount {
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}

impl ClientAccount {
    pub fn new() -> Self {
        Self {
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            total: Decimal::ZERO,
            locked: false,
        }
    }

    pub fn deposit(&mut self, amount: Decimal) {
        self.available += amount;
        self.total += amount;
    }

    pub fn withdraw(&mut self, amount: Decimal) -> bool {
        if self.available >= amount {
            self.available -= amount;
            self.total -= amount;
            true
        } else {
            false
        }
    }

    pub fn hold(&mut self, amount: Decimal) {
        self.available -= amount;
        self.held += amount;
    }

    pub fn release(&mut self, amount: Decimal) {
        self.held -= amount;
        self.available += amount;
    }

    pub fn chargeback(&mut self, amount: Decimal) {
        self.held -= amount;
        self.total -= amount;
        self.locked = true;
    }
}

#[derive(Debug, Serialize)]
pub struct OutputRecord {
    pub client: u16,
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}
