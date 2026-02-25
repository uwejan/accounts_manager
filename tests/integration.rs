use std::collections::HashMap;
use std::io::Cursor;

use rust_decimal::Decimal;
use std::str::FromStr;

/// Parse a decimal literal for test assertions.
fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

/// Account state returned by the test engine.
#[derive(Debug)]
struct AccountState {
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

/// Run the full engine pipeline on raw CSV input and return parsed output rows.
fn run_engine(csv_input: &str) -> HashMap<u16, AccountState> {
    use csv::ReaderBuilder;
    use serde::Deserialize;

    #[derive(Debug, Clone, Deserialize, PartialEq)]
    #[serde(rename_all = "lowercase")]
    enum TransactionType {
        Deposit,
        Withdrawal,
        Dispute,
        Resolve,
        Chargeback,
    }

    #[derive(Debug, Clone, Deserialize)]
    struct TransactionRecord {
        r#type: TransactionType,
        client: u16,
        tx: u32,
        amount: Option<Decimal>,
    }

    #[derive(Debug, Clone)]
    struct StoredTransaction {
        client: u16,
        amount: Decimal,
        under_dispute: bool,
    }

    #[derive(Debug, Clone)]
    struct ClientAccount {
        available: Decimal,
        held: Decimal,
        total: Decimal,
        locked: bool,
    }

    impl ClientAccount {
        fn new() -> Self {
            Self {
                available: Decimal::ZERO,
                held: Decimal::ZERO,
                total: Decimal::ZERO,
                locked: false,
            }
        }
        fn deposit(&mut self, amount: Decimal) {
            self.available += amount;
            self.total += amount;
        }
        fn withdraw(&mut self, amount: Decimal) -> bool {
            if self.available >= amount {
                self.available -= amount;
                self.total -= amount;
                true
            } else {
                false
            }
        }
        fn hold(&mut self, amount: Decimal) {
            self.available -= amount;
            self.held += amount;
        }
        fn release(&mut self, amount: Decimal) {
            self.held -= amount;
            self.available += amount;
        }
        fn chargeback(&mut self, amount: Decimal) {
            self.held -= amount;
            self.total -= amount;
            self.locked = true;
        }
    }

    let mut clients: HashMap<u16, ClientAccount> = HashMap::new();
    let mut transactions: HashMap<u32, StoredTransaction> = HashMap::new();

    let cursor = Cursor::new(csv_input);
    let mut reader = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(cursor);

    for result in reader.deserialize::<TransactionRecord>() {
        let record = match result {
            Ok(r) => r,
            Err(_) => continue,
        };

        match record.r#type {
            TransactionType::Deposit => {
                if let Some(amount) = record.amount {
                    let acct = clients
                        .entry(record.client)
                        .or_insert_with(ClientAccount::new);
                    if !acct.locked {
                        acct.deposit(amount);
                        transactions.insert(
                            record.tx,
                            StoredTransaction {
                                client: record.client,
                                amount,
                                under_dispute: false,
                            },
                        );
                    }
                }
            }
            TransactionType::Withdrawal => {
                if let Some(amount) = record.amount {
                    let acct = clients
                        .entry(record.client)
                        .or_insert_with(ClientAccount::new);
                    if !acct.locked {
                        acct.withdraw(amount);
                    }
                }
            }
            TransactionType::Dispute => {
                if let Some(stored) = transactions.get_mut(&record.tx) {
                    if stored.client != record.client {
                        continue;
                    }
                    if stored.under_dispute {
                        continue;
                    }
                    if let Some(acct) = clients.get_mut(&record.client) {
                        if acct.locked {
                            continue;
                        }
                        stored.under_dispute = true;
                        acct.hold(stored.amount);
                    }
                }
            }
            TransactionType::Resolve => {
                if let Some(stored) = transactions.get_mut(&record.tx) {
                    if stored.client != record.client {
                        continue;
                    }
                    if !stored.under_dispute {
                        continue;
                    }
                    if let Some(acct) = clients.get_mut(&record.client) {
                        if acct.locked {
                            continue;
                        }
                        stored.under_dispute = false;
                        acct.release(stored.amount);
                    }
                }
            }
            TransactionType::Chargeback => {
                if let Some(stored) = transactions.get_mut(&record.tx) {
                    if stored.client != record.client {
                        continue;
                    }
                    if !stored.under_dispute {
                        continue;
                    }
                    if let Some(acct) = clients.get_mut(&record.client) {
                        if acct.locked {
                            continue;
                        }
                        stored.under_dispute = false;
                        acct.chargeback(stored.amount);
                    }
                }
            }
        }
    }

    let mut result_map = HashMap::new();
    for (&cid, acct) in &clients {
        result_map.insert(
            cid,
            AccountState {
                available: acct.available,
                held: acct.held,
                total: acct.total,
                locked: acct.locked,
            },
        );
    }
    result_map
}

// ─── Test Cases ──────────────────────────────────────────────────────────────

#[test]
fn test_basic_deposits_and_withdrawals() {
    let input = "\
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0
";
    let out = run_engine(input);

    let c1 = &out[&1];
    assert_eq!(c1.available, dec("1.5"));
    assert_eq!(c1.held, dec("0"));
    assert_eq!(c1.total, dec("1.5"));
    assert!(!c1.locked);

    let c2 = &out[&2];
    assert_eq!(c2.available, dec("2.0"));
    assert_eq!(c2.held, dec("0"));
    assert_eq!(c2.total, dec("2.0"));
    assert!(!c2.locked);
}

#[test]
fn test_dispute_then_resolve() {
    let input = "\
type, client, tx, amount
deposit, 1, 1, 10.0
dispute, 1, 1,
resolve, 1, 1,
";
    let out = run_engine(input);
    let c1 = &out[&1];
    assert_eq!(c1.available, dec("10.0"));
    assert_eq!(c1.held, dec("0"));
    assert_eq!(c1.total, dec("10.0"));
    assert!(!c1.locked);
}

#[test]
fn test_dispute_then_chargeback() {
    let input = "\
type, client, tx, amount
deposit, 1, 1, 10.0
dispute, 1, 1,
chargeback, 1, 1,
";
    let out = run_engine(input);
    let c1 = &out[&1];
    assert_eq!(c1.available, dec("0"));
    assert_eq!(c1.held, dec("0"));
    assert_eq!(c1.total, dec("0"));
    assert!(c1.locked);
}

#[test]
fn test_dispute_nonexistent_tx_ignored() {
    let input = "\
type, client, tx, amount
deposit, 1, 1, 5.0
dispute, 1, 999,
";
    let out = run_engine(input);
    let c1 = &out[&1];
    assert_eq!(c1.available, dec("5.0"));
    assert_eq!(c1.held, dec("0"));
    assert_eq!(c1.total, dec("5.0"));
    assert!(!c1.locked);
}

#[test]
fn test_resolve_without_dispute_ignored() {
    let input = "\
type, client, tx, amount
deposit, 1, 1, 5.0
resolve, 1, 1,
";
    let out = run_engine(input);
    let c1 = &out[&1];
    assert_eq!(c1.available, dec("5.0"));
    assert_eq!(c1.held, dec("0"));
}

#[test]
fn test_chargeback_without_dispute_ignored() {
    let input = "\
type, client, tx, amount
deposit, 1, 1, 5.0
chargeback, 1, 1,
";
    let out = run_engine(input);
    let c1 = &out[&1];
    assert_eq!(c1.available, dec("5.0"));
    assert_eq!(c1.held, dec("0"));
    assert!(!c1.locked);
}

#[test]
fn test_locked_account_ignores_deposits_and_withdrawals() {
    let input = "\
type, client, tx, amount
deposit, 1, 1, 10.0
dispute, 1, 1,
chargeback, 1, 1,
deposit, 1, 2, 50.0
withdrawal, 1, 3, 1.0
";
    let out = run_engine(input);
    let c1 = &out[&1];
    assert_eq!(c1.available, dec("0"));
    assert_eq!(c1.total, dec("0"));
    assert!(c1.locked);
}

#[test]
fn test_locked_account_rejects_disputes() {
    let input = "\
type, client, tx, amount
deposit, 1, 1, 10.0
deposit, 1, 2, 5.0
dispute, 1, 1,
chargeback, 1, 1,
dispute, 1, 2,
";
    let out = run_engine(input);
    let c1 = &out[&1];
    // Dispute on tx 2 should be ignored because account is locked
    assert_eq!(c1.available, dec("5.0"));
    assert_eq!(c1.held, dec("0"));
    assert_eq!(c1.total, dec("5.0"));
    assert!(c1.locked);
}

#[test]
fn test_decimal_precision() {
    let input = "\
type, client, tx, amount
deposit, 1, 1, 1.1111
deposit, 1, 2, 2.2222
withdrawal, 1, 3, 0.3333
";
    let out = run_engine(input);
    let c1 = &out[&1];
    assert_eq!(c1.available, dec("3.0000"));
    assert_eq!(c1.total, dec("3.0000"));
}

#[test]
fn test_multiple_clients_interleaved() {
    let input = "\
type, client, tx, amount
deposit, 2, 1, 100.0
deposit, 1, 2, 50.0
withdrawal, 2, 3, 25.0
deposit, 1, 4, 25.0
";
    let out = run_engine(input);

    let c1 = &out[&1];
    assert_eq!(c1.available, dec("75.0"));
    assert_eq!(c1.total, dec("75.0"));

    let c2 = &out[&2];
    assert_eq!(c2.available, dec("75.0"));
    assert_eq!(c2.total, dec("75.0"));
}

#[test]
fn test_whitespace_tolerance() {
    let input = "\
type ,  client ,  tx ,  amount
deposit ,  1 ,  1 ,  5.0
withdrawal ,  1 ,  2 ,  2.0
";
    let out = run_engine(input);
    let c1 = &out[&1];
    assert_eq!(c1.available, dec("3.0"));
    assert_eq!(c1.total, dec("3.0"));
}

#[test]
fn test_dispute_wrong_client_ignored() {
    let input = "\
type, client, tx, amount
deposit, 1, 1, 10.0
dispute, 2, 1,
";
    let out = run_engine(input);
    let c1 = &out[&1];
    assert_eq!(c1.available, dec("10.0"));
    assert_eq!(c1.held, dec("0"));
}
