mod engine;
mod types;

use std::env;
use std::fs::File;
use std::io::{self, BufReader, Write};
use std::process;

use crate::engine::PaymentsEngine;
use crate::types::{FormattedDecimal, InputRecord, OutputRecord};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <transactions.csv>", args[0]);
        process::exit(1);
    }

    let file = File::open(&args[1])?;
    let reader = BufReader::new(file);

    let mut csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(reader);

    let mut engine = PaymentsEngine::new();

    for result in csv_reader.deserialize::<InputRecord>() {
        match result {
            Ok(record) => engine.process(record),
            Err(e) => {
                eprintln!("warning: skipping malformed record: {}", e);
            }
        }
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();

    writeln!(out, "client,available,held,total,locked")?;

    for (&client, account) in engine.accounts() {
        let record = OutputRecord {
            client,
            available: FormattedDecimal(account.available),
            held: FormattedDecimal(account.held),
            total: FormattedDecimal(account.total()),
            locked: account.locked,
        };

        writeln!(
            out,
            "{},{},{},{},{}",
            record.client, record.available, record.held, record.total, record.locked
        )?;
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {}", e);
        process::exit(1);
    }
}
