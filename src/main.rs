//! # Accounts Manager
//!
//! A toy payments engine that processes CSV transaction records
//! (deposits, withdrawals, disputes, resolves, chargebacks)
//! and outputs the final state of all client accounts.
//!
//! ## Author
//!
//! Saddam (Sam) Uwejan

mod engine;
mod error;
mod types;

use std::fs::File;
use std::process;

use clap::Parser;
use csv::ReaderBuilder;

use engine::PaymentsEngine;
use error::EngineError;
use types::TransactionRecord;

#[derive(Parser)]
#[command(name = "accounts_manager", author = "Saddam Uwejan")]
#[command(about = "Process payment transactions and output client account states")]
struct Cli {
    input_file: String,
}

fn run() -> Result<(), EngineError> {
    let cli = Cli::parse();

    let file = File::open(&cli.input_file)?;
    let mut reader = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(false)
        .from_reader(file);

    let mut engine = PaymentsEngine::new();

    for result in reader.deserialize::<TransactionRecord>() {
        match result {
            Ok(record) => engine.process(record),
            Err(e) => {
                eprintln!("warning: skipping malformed row: {e}");
            }
        }
    }

    engine.write_output(std::io::stdout())?;

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
