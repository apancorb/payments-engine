use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::types::{Account, InputRecord, StoredTransaction, TransactionType};

/// The payments engine that processes transactions and maintains account state.
pub struct PaymentsEngine {
    accounts: HashMap<u16, Account>,
    transactions: HashMap<u32, StoredTransaction>,
}

impl PaymentsEngine {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
        }
    }

    /// Process a single transaction record. Errors in the record are silently ignored
    /// per the spec (partner-side errors).
    pub fn process(&mut self, record: InputRecord) {
        match record.r#type {
            TransactionType::Deposit => self.deposit(record.client, record.tx, record.amount),
            TransactionType::Withdrawal => self.withdrawal(record.client, record.tx, record.amount),
            TransactionType::Dispute => self.dispute(record.client, record.tx),
            TransactionType::Resolve => self.resolve(record.client, record.tx),
            TransactionType::Chargeback => self.chargeback(record.client, record.tx),
        }
    }

    /// Returns an iterator over all client accounts.
    pub fn accounts(&self) -> impl Iterator<Item = (&u16, &Account)> {
        self.accounts.iter()
    }

    fn get_or_create_account(&mut self, client: u16) -> &mut Account {
        self.accounts.entry(client).or_insert_with(Account::new)
    }

    fn deposit(&mut self, client: u16, tx: u32, amount: Option<Decimal>) {
        let amount = match amount {
            Some(a) if a > Decimal::ZERO => a,
            _ => return,
        };

        // Ignore duplicate tx IDs
        if self.transactions.contains_key(&tx) {
            return;
        }

        let account = self.get_or_create_account(client);
        if account.locked {
            return;
        }

        account.available += amount;

        self.transactions.insert(
            tx,
            StoredTransaction {
                client,
                amount,
                disputed: false,
                chargebacked: false,
            },
        );
    }

    fn withdrawal(&mut self, client: u16, tx: u32, amount: Option<Decimal>) {
        let amount = match amount {
            Some(a) if a > Decimal::ZERO => a,
            _ => return,
        };

        // Ignore duplicate tx IDs
        if self.transactions.contains_key(&tx) {
            return;
        }

        let account = self.get_or_create_account(client);
        if account.locked {
            return;
        }

        if account.available < amount {
            return;
        }

        account.available -= amount;
    }

    fn dispute(&mut self, client: u16, tx: u32) {
        let stored = match self.transactions.get_mut(&tx) {
            Some(t) if t.client == client && !t.disputed && !t.chargebacked => t,
            _ => return,
        };

        stored.disputed = true;
        let amount = stored.amount;

        let account = self.get_or_create_account(client);
        account.available -= amount;
        account.held += amount;
    }

    fn resolve(&mut self, client: u16, tx: u32) {
        let stored = match self.transactions.get_mut(&tx) {
            Some(t) if t.client == client && t.disputed => t,
            _ => return,
        };

        stored.disputed = false;
        let amount = stored.amount;

        let account = self.get_or_create_account(client);
        account.held -= amount;
        account.available += amount;
    }

    fn chargeback(&mut self, client: u16, tx: u32) {
        let stored = match self.transactions.get_mut(&tx) {
            Some(t) if t.client == client && t.disputed => t,
            _ => return,
        };

        stored.disputed = false;
        stored.chargebacked = true;
        let amount = stored.amount;

        let account = self.get_or_create_account(client);
        account.held -= amount;
        account.locked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn deposit_record(client: u16, tx: u32, amount: Decimal) -> InputRecord {
        InputRecord {
            r#type: TransactionType::Deposit,
            client,
            tx,
            amount: Some(amount),
        }
    }

    fn withdrawal_record(client: u16, tx: u32, amount: Decimal) -> InputRecord {
        InputRecord {
            r#type: TransactionType::Withdrawal,
            client,
            tx,
            amount: Some(amount),
        }
    }

    fn dispute_record(client: u16, tx: u32) -> InputRecord {
        InputRecord {
            r#type: TransactionType::Dispute,
            client,
            tx,
            amount: None,
        }
    }

    fn resolve_record(client: u16, tx: u32) -> InputRecord {
        InputRecord {
            r#type: TransactionType::Resolve,
            client,
            tx,
            amount: None,
        }
    }

    fn chargeback_record(client: u16, tx: u32) -> InputRecord {
        InputRecord {
            r#type: TransactionType::Chargeback,
            client,
            tx,
            amount: None,
        }
    }

    fn get_account(engine: &PaymentsEngine, client: u16) -> &Account {
        engine.accounts.get(&client).expect("account should exist")
    }

    #[test]
    fn test_single_deposit() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(10.0));
        assert!(!account.locked);
    }

    #[test]
    fn test_multiple_deposits_same_client() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(deposit_record(1, 2, dec!(5.5)));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(15.5));
        assert_eq!(account.total(), dec!(15.5));
    }

    #[test]
    fn test_withdrawal_success() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(withdrawal_record(1, 2, dec!(3.0)));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(7.0));
        assert_eq!(account.total(), dec!(7.0));
    }

    #[test]
    fn test_withdrawal_insufficient_funds() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(5.0)));
        engine.process(withdrawal_record(1, 2, dec!(10.0)));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(5.0));
        assert_eq!(account.total(), dec!(5.0));
    }

    #[test]
    fn test_withdrawal_exact_balance() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(5.0)));
        engine.process(withdrawal_record(1, 2, dec!(5.0)));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.total(), dec!(0));
    }

    #[test]
    fn test_dispute() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(dispute_record(1, 1));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(10.0));
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn test_resolve() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(dispute_record(1, 1));
        engine.process(resolve_record(1, 1));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn test_chargeback() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(dispute_record(1, 1));
        engine.process(chargeback_record(1, 1));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(0));
        assert!(account.locked);
    }

    #[test]
    fn test_dispute_nonexistent_tx() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(dispute_record(1, 999));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.held, dec!(0));
    }

    #[test]
    fn test_resolve_non_disputed_tx() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(resolve_record(1, 1));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.held, dec!(0));
    }

    #[test]
    fn test_chargeback_non_disputed_tx() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(chargeback_record(1, 1));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(10.0));
        assert!(!account.locked);
    }

    #[test]
    fn test_dispute_wrong_client() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(deposit_record(2, 2, dec!(5.0)));
        // Client 2 tries to dispute client 1's transaction
        engine.process(dispute_record(2, 1));

        let account1 = get_account(&engine, 1);
        assert_eq!(account1.available, dec!(10.0));
        assert_eq!(account1.held, dec!(0));

        let account2 = get_account(&engine, 2);
        assert_eq!(account2.available, dec!(5.0));
        assert_eq!(account2.held, dec!(0));
    }

    #[test]
    fn test_locked_account_rejects_deposit() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(dispute_record(1, 1));
        engine.process(chargeback_record(1, 1));
        // Account is now locked
        engine.process(deposit_record(1, 2, dec!(5.0)));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.total(), dec!(0));
        assert!(account.locked);
    }

    #[test]
    fn test_locked_account_rejects_withdrawal() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(deposit_record(1, 2, dec!(5.0)));
        engine.process(dispute_record(1, 1));
        engine.process(chargeback_record(1, 1));
        // Account locked, still has 5.0 available
        engine.process(withdrawal_record(1, 3, dec!(5.0)));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(5.0));
        assert!(account.locked);
    }

    #[test]
    fn test_duplicate_tx_id_ignored() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(deposit_record(1, 1, dec!(20.0))); // duplicate

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(10.0));
    }

    #[test]
    fn test_precision_four_decimal_places() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(1.2345)));
        engine.process(deposit_record(1, 2, dec!(2.3456)));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(3.5801));
    }

    #[test]
    fn test_dispute_after_partial_withdrawal() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(withdrawal_record(1, 2, dec!(7.0)));
        // Available is 3.0, now dispute the 10.0 deposit
        engine.process(dispute_record(1, 1));

        let account = get_account(&engine, 1);
        // available goes negative: 3.0 - 10.0 = -7.0
        assert_eq!(account.available, dec!(-7.0));
        assert_eq!(account.held, dec!(10.0));
        assert_eq!(account.total(), dec!(3.0));
    }

    #[test]
    fn test_double_dispute_same_tx() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(dispute_record(1, 1));
        // Second dispute on already-disputed tx should be ignored
        engine.process(dispute_record(1, 1));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(10.0));
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn test_resolve_then_re_dispute() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(dispute_record(1, 1));
        engine.process(resolve_record(1, 1));
        // After resolve, should be able to dispute again
        engine.process(dispute_record(1, 1));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(10.0));
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn test_chargeback_prevents_re_dispute() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(deposit_record(1, 2, dec!(5.0)));
        engine.process(dispute_record(1, 1));
        engine.process(chargeback_record(1, 1));
        // Chargeback is the final state — re-dispute must be rejected
        engine.process(dispute_record(1, 1));

        let account = get_account(&engine, 1);
        // available=5 (from tx2), held=0 (chargeback removed held), total=5
        assert_eq!(account.available, dec!(5.0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(5.0));
        assert!(account.locked);
    }

    #[test]
    fn test_multiple_disputes_different_txs() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(10.0)));
        engine.process(deposit_record(1, 2, dec!(20.0)));
        engine.process(dispute_record(1, 1));
        engine.process(dispute_record(1, 2));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(30.0));
        assert_eq!(account.total(), dec!(30.0));
    }

    #[test]
    fn test_deposit_zero_amount_ignored() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(0.0)));

        assert!(engine.accounts.get(&1).is_none());
    }

    #[test]
    fn test_withdrawal_without_prior_deposit() {
        let mut engine = PaymentsEngine::new();
        engine.process(withdrawal_record(1, 1, dec!(10.0)));

        let account = get_account(&engine, 1);
        assert_eq!(account.available, dec!(0));
    }

    #[test]
    fn test_pdf_example() {
        let mut engine = PaymentsEngine::new();
        engine.process(deposit_record(1, 1, dec!(1.0)));
        engine.process(deposit_record(2, 2, dec!(2.0)));
        engine.process(deposit_record(1, 3, dec!(2.0)));
        engine.process(withdrawal_record(1, 4, dec!(1.5)));
        engine.process(withdrawal_record(2, 5, dec!(3.0)));

        let a1 = get_account(&engine, 1);
        assert_eq!(a1.available, dec!(1.5));
        assert_eq!(a1.held, dec!(0));
        assert_eq!(a1.total(), dec!(1.5));
        assert!(!a1.locked);

        let a2 = get_account(&engine, 2);
        assert_eq!(a2.available, dec!(2.0));
        assert_eq!(a2.held, dec!(0));
        assert_eq!(a2.total(), dec!(2.0));
        assert!(!a2.locked);
    }
}
