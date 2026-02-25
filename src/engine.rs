use std::collections::HashMap;
use std::io;

use crate::error::EngineError;
use crate::types::{
    ClientAccount, OutputRecord, StoredTransaction, TransactionRecord, TransactionType,
};

/// Maintains client accounts and stored deposit transactions for dispute lookups.
pub struct PaymentsEngine {
    clients: HashMap<u16, ClientAccount>,
    transactions: HashMap<u32, StoredTransaction>,
}

impl PaymentsEngine {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            transactions: HashMap::new(),
        }
    }

    pub fn process(&mut self, record: TransactionRecord) {
        match record.r#type {
            TransactionType::Deposit => self.handle_deposit(record),
            TransactionType::Withdrawal => self.handle_withdrawal(record),
            TransactionType::Dispute => self.handle_dispute(record),
            TransactionType::Resolve => self.handle_resolve(record),
            TransactionType::Chargeback => self.handle_chargeback(record),
        }
    }

    fn handle_deposit(&mut self, record: TransactionRecord) {
        if let Some(amount) = record.amount {
            let account = self
                .clients
                .entry(record.client)
                .or_insert_with(ClientAccount::new);

            if account.locked {
                return;
            }

            account.deposit(amount);

            // Store deposit metadata for future dispute lookups
            self.transactions.insert(
                record.tx,
                StoredTransaction {
                    client: record.client,
                    amount,
                    under_dispute: false,
                },
            );
        }
    }

    fn handle_withdrawal(&mut self, record: TransactionRecord) {
        if let Some(amount) = record.amount {
            let account = self
                .clients
                .entry(record.client)
                .or_insert_with(ClientAccount::new);

            if account.locked {
                return;
            }

            account.withdraw(amount);
        }
    }

    fn handle_dispute(&mut self, record: TransactionRecord) {
        if let Some(stored) = self.transactions.get_mut(&record.tx) {
            if stored.client != record.client {
                return;
            }

            // Prevent double-disputes would incorrectly drain available into held
            if stored.under_dispute {
                return;
            }

            if let Some(account) = self.clients.get_mut(&record.client) {
                if account.locked {
                    return;
                }

                stored.under_dispute = true;
                account.hold(stored.amount);
            }
        }
    }

    fn handle_resolve(&mut self, record: TransactionRecord) {
        if let Some(stored) = self.transactions.get_mut(&record.tx) {
            if stored.client != record.client {
                return;
            }

            // Can only resolve a transaction that is currently under dispute
            if !stored.under_dispute {
                return;
            }

            if let Some(account) = self.clients.get_mut(&record.client) {
                if account.locked {
                    return;
                }

                stored.under_dispute = false;
                account.release(stored.amount);
            }
        }
    }

    fn handle_chargeback(&mut self, record: TransactionRecord) {
        if let Some(stored) = self.transactions.get_mut(&record.tx) {
            if stored.client != record.client {
                return;
            }

            // Can only chargeback a transaction that is currently under dispute
            if !stored.under_dispute {
                return;
            }

            if let Some(account) = self.clients.get_mut(&record.client) {
                if account.locked {
                    return;
                }

                stored.under_dispute = false;
                account.chargeback(stored.amount);
            }
        }
    }

    pub fn write_output<W: io::Write>(&self, writer: W) -> Result<(), EngineError> {
        let mut wtr = csv::Writer::from_writer(writer);

        for (&client_id, account) in &self.clients {
            wtr.serialize(OutputRecord {
                client: client_id,
                available: account.available,
                held: account.held,
                total: account.total,
                locked: account.locked,
            })?;
        }

        wtr.flush()?;
        Ok(())
    }
}
