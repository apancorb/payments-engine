use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// The type of transaction being processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

/// A single row from the input CSV.
#[derive(Debug, Clone, Deserialize)]
pub struct InputRecord {
    pub r#type: TransactionType,
    pub client: u16,
    pub tx: u32,
    pub amount: Option<Decimal>,
}

/// A deposit stored for potential dispute lookups.
#[derive(Debug, Clone)]
pub struct StoredTransaction {
    pub client: u16,
    pub amount: Decimal,
    pub disputed: bool,
    pub chargebacked: bool,
}

/// A client's account state.
#[derive(Debug, Clone)]
pub struct Account {
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
}

impl Account {
    pub fn new() -> Self {
        Self {
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
        }
    }

    pub fn total(&self) -> Decimal {
        self.available + self.held
    }
}

/// A single row for the output CSV.
#[derive(Debug, Serialize)]
pub struct OutputRecord {
    pub client: u16,
    pub available: FormattedDecimal,
    pub held: FormattedDecimal,
    pub total: FormattedDecimal,
    pub locked: bool,
}

/// Wrapper to serialize Decimal with exactly 4 decimal places.
#[derive(Debug, Clone)]
pub struct FormattedDecimal(pub Decimal);

impl Serialize for FormattedDecimal {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:.4}", self.0))
    }
}
