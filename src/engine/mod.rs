use std::collections::HashMap;

use tokio_stream::{Stream, StreamExt};
use tracing::info;

use crate::Amount;
use crate::model::{ClientId, Transaction, TxId};

mod state;
use state::ClientAccount;

mod error;
pub use error::{DepositError, EngineError, WithdrawalError};

pub struct Engine {
    clients: HashMap<ClientId, ClientAccount>,
}

/// Public API
impl Engine {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    /// Run the engine with the given transaction stream
    pub async fn run(&mut self, mut stream: impl Stream<Item = Transaction> + Unpin) {
        while let Some(tx) = stream.next().await {
            // any error should not stop the engine, so we just ignore the application result
            let _ = self.apply(tx);
        }
    }

    /// Return the state of client accounts flattened by client identifier
    pub fn clients(&self) -> impl Iterator<Item = (ClientId, Amount, Amount, Amount, bool)> + '_ {
        self.clients.iter().map(|(id, account)| {
            (
                *id,
                account.available,
                account.held,
                account.total(),
                account.frozen,
            )
        })
    }

    pub fn get_client(&self, client: ClientId) -> Option<&ClientAccount> {
        self.clients.get(&client)
    }
}

/// Private API
impl Engine {
    /// Apply the given transaction on top of the current engine state
    fn apply(&mut self, tx: Transaction) -> Result<(), EngineError> {
        match &tx {
            Transaction::Deposit { client, tx, amount } => {
                let result = self.apply_deposit(*client, *amount);
                Self::log_result("deposit", *client, *tx, *amount, &result);
                result?;
            }
            Transaction::Withdrawal { client, tx, amount } => {
                let result = self.apply_withdrawal(*client, *amount);
                Self::log_result("withdrawal", *client, *tx, *amount, &result);
                result?;
            }
        }
        Ok(())
    }

    /// Small helper to log `apply` results
    fn log_result<E: std::fmt::Display>(
        tx_type: &str,
        client: ClientId,
        tx: TxId,
        amount: Amount,
        result: &Result<(), E>,
    ) {
        match result {
            Ok(()) => {
                info!(
                    client = %client,
                    tx = %tx,
                    amount = %amount,
                    "{tx_type} applied"
                );
            }
            Err(e) => {
                info!(
                    client = %client,
                    tx = %tx,
                    amount = %amount,
                    reason = %e,
                    "{tx_type} skipped"
                );
            }
        }
    }

    /// Apply a `Transaction::Deposit`:
    /// - Ensure account is unfrozen
    /// - Increment account available balance by the deposit amount
    fn apply_deposit(&mut self, client: ClientId, amount: Amount) -> Result<(), DepositError> {
        let account = self.clients.entry(client).or_default();
        if account.frozen {
            return Err(DepositError::AccountFrozen(client));
        }

        account.available += amount;
        Ok(())
    }

    /// Apply a `Transaction::Withdrawal`:
    /// - Ensure account is unfrozen and has enough available balance
    /// - Decrement account available balance by the withdrawal amount
    fn apply_withdrawal(
        &mut self,
        client: ClientId,
        amount: Amount,
    ) -> Result<(), WithdrawalError> {
        let account = self.clients.entry(client).or_default();
        if account.frozen {
            return Err(WithdrawalError::AccountFrozen(client));
        }

        if account.available < amount {
            return Err(WithdrawalError::InsufficientFunds(
                client,
                account.available,
                amount,
            ));
        }

        account.available -= amount;
        Ok(())
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // test utils

    fn deposit(client: ClientId, tx: TxId, amount: i64) -> Transaction {
        Transaction::Deposit {
            client,
            tx,
            amount: Amount::from_scaled(amount),
        }
    }

    fn withdrawal(client: ClientId, tx: TxId, amount: i64) -> Transaction {
        Transaction::Withdrawal {
            client,
            tx,
            amount: Amount::from_scaled(amount),
        }
    }

    #[test]
    fn new_engine() {
        let engine = Engine::new();
        assert_eq!(engine.clients().count(), 0);
    }

    // Deposit

    #[test]
    fn deposit_creates_account_and_increases_balance() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();

        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available, Amount::from_scaled(100));
        assert_eq!(client.held, Amount::from_scaled(0));
        assert!(!client.frozen);
    }

    #[test]
    fn deposit_accumulates_balance() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(deposit(1, 2, 50)).unwrap();

        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available, Amount::from_scaled(150));
    }

    #[test]
    fn deposit_to_frozen_account_fails() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.clients.get_mut(&1).unwrap().freeze();

        let result = engine.apply(deposit(1, 2, 50));
        assert!(matches!(
            result,
            Err(EngineError::Deposit(DepositError::AccountFrozen(1)))
        ));

        // Balance unchanged
        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available, Amount::from_scaled(100));
    }

    // Withdrawal

    #[test]
    fn withdrawal_decreases_balance() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(withdrawal(1, 2, 30)).unwrap();

        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available, Amount::from_scaled(70));
    }

    #[test]
    fn withdrawal_exact_amount_succeeds() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(withdrawal(1, 2, 100)).unwrap();

        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available, Amount::from_scaled(0));
    }

    #[test]
    fn withdrawal_insufficient_funds_fails() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();

        let result = engine.apply(withdrawal(1, 2, 101));
        assert!(matches!(
            result,
            Err(EngineError::Withdrawal(WithdrawalError::InsufficientFunds(
                1,
                _,
                _
            )))
        ));

        // Balance unchanged
        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available, Amount::from_scaled(100));
    }

    #[test]
    fn withdrawal_from_frozen_account_fails() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.clients.get_mut(&1).unwrap().freeze();

        let result = engine.apply(withdrawal(1, 2, 50));
        assert!(matches!(
            result,
            Err(EngineError::Withdrawal(WithdrawalError::AccountFrozen(1)))
        ));

        // Balance unchanged
        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available, Amount::from_scaled(100));
    }

    #[test]
    fn withdrawal_from_nonexistent_account_fails() {
        let mut engine = Engine::new();

        // equivalent to zero account
        let result = engine.apply(withdrawal(1, 1, 50));
        assert!(matches!(
            result,
            Err(EngineError::Withdrawal(WithdrawalError::InsufficientFunds(
                1,
                _,
                _
            )))
        ));
    }

    // Multiple Clients

    #[test]
    fn multiple_clients_are_independent() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(deposit(2, 2, 200)).unwrap();
        engine.apply(withdrawal(1, 3, 30)).unwrap();

        let client1 = engine.get_client(1).unwrap();
        let client2 = engine.get_client(2).unwrap();

        assert_eq!(client1.available, Amount::from_scaled(70));
        assert_eq!(client2.available, Amount::from_scaled(200));
    }

    // clients() iterator

    #[test]
    fn clients_iterator_returns_all_clients() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(deposit(2, 2, 200)).unwrap();

        let clients: Vec<_> = engine.clients().collect();
        assert_eq!(clients.len(), 2);

        // Find each client (without any ordering guarantees)
        let c1 = clients.iter().find(|(id, _, _, _, _)| *id == 1).unwrap();
        let c2 = clients.iter().find(|(id, _, _, _, _)| *id == 2).unwrap();

        assert_eq!(c1.1, Amount::from_scaled(100)); // available
        assert_eq!(c2.1, Amount::from_scaled(200)); // available
    }

    //  Async run()

    #[tokio::test]
    async fn run_processes_all_transactions() {
        let mut engine = Engine::new();
        let transactions = vec![deposit(1, 1, 100), deposit(2, 2, 200), withdrawal(1, 3, 25)];

        engine.run(tokio_stream::iter(transactions)).await;

        let client1 = engine.get_client(1).unwrap();
        let client2 = engine.get_client(2).unwrap();

        assert_eq!(client1.available, Amount::from_scaled(75));
        assert_eq!(client2.available, Amount::from_scaled(200));
    }

    #[tokio::test]
    async fn run_skips_failed_transactions_and_continues() {
        let mut engine = Engine::new();
        let transactions = vec![
            deposit(1, 1, 100),
            withdrawal(1, 2, 200), // Should fail with insufficient funds
            deposit(1, 3, 50),     // Should still process
        ];

        engine.run(tokio_stream::iter(transactions)).await;

        let client = engine.get_client(1).unwrap();

        assert_eq!(client.available, Amount::from_scaled(150)); // 100 + 50 with withdrawal skipped
    }
}
