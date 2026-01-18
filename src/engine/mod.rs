//! Transaction processing engine.
//!
//! The engine processes transactions and maintains client account state.
//! It supports deposits, withdrawals, disputes, resolutions, and chargebacks.
//! Also supports async stream of transactions.

use std::collections::{HashMap, HashSet};
use tokio_stream::{Stream, StreamExt};
use tracing::{info, warn};

use crate::Amount;
use crate::model::{ClientId, DepositRecord, DepositState, Transaction, TxId};

mod state;
pub use state::ClientAccount;

mod error;
pub use error::{
    DepositError, DepositOperation, DepositOperationError, EngineError, WithdrawalError,
};

/// The transaction processing engine.
///
/// Maintains client accounts and deposit records for dispute tracking.
pub struct Engine {
    clients: HashMap<ClientId, ClientAccount>,
    /// Deposit records for dispute tracking (chargedback deposits are evicted)
    deposits: HashMap<TxId, DepositRecord>,
    /// Track withdrawal tx IDs for duplicate checking only
    withdrawal_ids: HashSet<TxId>,
}

/// Public API
impl Engine {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            deposits: HashMap::new(),
            withdrawal_ids: HashSet::new(),
        }
    }

    /// Run the engine with the given transaction stream
    pub async fn run(&mut self, mut stream: impl Stream<Item = Transaction> + Unpin) {
        while let Some(tx) = stream.next().await {
            // any error should not stop the engine, so we just ignore the application result
            let _ = self.apply(tx);
        }
    }

    /// Return the state of client accounts.
    pub fn clients(&self) -> impl Iterator<Item = &ClientAccount> + '_ {
        self.clients.values()
    }

    /// Return the state of one client account
    pub fn get_client(&self, client: ClientId) -> Option<&ClientAccount> {
        self.clients.get(&client)
    }

    /// Apply a single transaction on top of the current engine state
    pub fn apply(&mut self, tx: Transaction) -> Result<(), EngineError> {
        match &tx {
            Transaction::Deposit { client, tx, amount } => {
                let result = self.apply_deposit(*client, *tx, *amount);
                Self::log_result("deposit", *client, *tx, Some(*amount), &result);
                result?;
            }
            Transaction::Withdrawal { client, tx, amount } => {
                let result = self.apply_withdrawal(*client, *tx, *amount);
                Self::log_result("withdrawal", *client, *tx, Some(*amount), &result);
                result?;
            }
            Transaction::Dispute { client, tx } => {
                let result = self.apply_dispute(*client, *tx);
                Self::log_result("dispute", *client, *tx, None, &result);
                result?;
            }
            Transaction::Resolve { client, tx } => {
                let result = self.apply_resolve(*client, *tx);
                Self::log_result("resolve", *client, *tx, None, &result);
                result?;
            }
            Transaction::Chargeback { client, tx } => {
                let result = self.apply_chargeback(*client, *tx);
                Self::log_result("chargeback", *client, *tx, None, &result);
                result?;
            }
        }
        Ok(())
    }
}

/// Private API
impl Engine {
    /// Small helper to log `apply` results
    fn log_result<E: std::fmt::Display>(
        tx_type: &str,
        client: ClientId,
        tx: TxId,
        amount: Option<Amount>,
        result: &Result<(), E>,
    ) {
        match (result, amount) {
            (Ok(()), Some(amt)) => {
                info!(
                    client = %client,
                    tx = %tx,
                    amount = %amt,
                    "{tx_type} applied"
                );
            }
            (Ok(()), None) => {
                info!(
                    client = %client,
                    tx = %tx,
                    "{tx_type} applied"
                );
            }
            (Err(e), Some(amt)) => {
                info!(
                    client = %client,
                    tx = %tx,
                    amount = %amt,
                    reason = %e,
                    "{tx_type} skipped"
                );
            }
            (Err(e), None) => {
                info!(
                    client = %client,
                    tx = %tx,
                    reason = %e,
                    "{tx_type} skipped"
                );
            }
        }
    }

    /// Ensure transaction ID is unique
    fn is_unique(&self, tx: &TxId) -> bool {
        !self.deposits.contains_key(tx) && !self.withdrawal_ids.contains(tx)
    }

    /// Apply a `Transaction::Deposit`:
    /// - Ensure transaction ID is unique
    /// - Ensure account is unfrozen
    /// - Increment account available balance by the deposit amount
    /// - Store deposit for potential disputes
    fn apply_deposit(
        &mut self,
        client: ClientId,
        tx: TxId,
        amount: Amount,
    ) -> Result<(), DepositError> {
        if !self.is_unique(&tx) {
            return Err(DepositError::DuplicateTxId(tx));
        }

        let account = self
            .clients
            .entry(client)
            .or_insert_with(|| ClientAccount::new(client));

        if account.is_frozen() {
            return Err(DepositError::AccountFrozen(client));
        }

        account.credit(amount);

        // Store deposit for potential disputes
        self.deposits.insert(tx, DepositRecord::new(client, amount));

        Ok(())
    }

    /// Apply a `Transaction::Withdrawal`:
    /// - Ensure transaction ID is unique
    /// - Ensure account is unfrozen and has enough available balance
    /// - Decrement account available balance by the withdrawal amount
    fn apply_withdrawal(
        &mut self,
        client: ClientId,
        tx: TxId,
        amount: Amount,
    ) -> Result<(), WithdrawalError> {
        if !self.is_unique(&tx) {
            return Err(WithdrawalError::DuplicateTxId(tx));
        }

        let account = self
            .clients
            .entry(client)
            .or_insert_with(|| ClientAccount::new(client));

        if account.is_frozen() {
            return Err(WithdrawalError::AccountFrozen(client));
        }

        if account.available() < amount {
            return Err(WithdrawalError::InsufficientFunds(
                client,
                account.available(),
                amount,
            ));
        }

        account.debit(amount);

        // Store only tx ID for duplicate checking (as withdrawals can't be disputed)
        self.withdrawal_ids.insert(tx);

        Ok(())
    }

    /// Apply a `Transaction::Dispute`:
    /// - Find the referenced deposit
    /// - Validate client ownership
    /// - Check deposit is in Ok state
    /// - Move funds from available to held
    ///
    /// Note: Disputes may result in negative available balance if funds were
    /// already withdrawn. This represents debt owed by the client.
    fn apply_dispute(&mut self, client: ClientId, tx: TxId) -> Result<(), DepositOperationError> {
        use DepositOperation::Dispute;

        // Only deposits can be disputed; other transaction types return "not found"
        let record = self
            .deposits
            .get_mut(&tx)
            .ok_or(DepositOperationError::TxNotFound(Dispute, tx))?;

        // Validate client ownership
        if record.client != client {
            return Err(DepositOperationError::ClientMismatch(
                Dispute,
                tx,
                record.client,
                client,
            ));
        }

        // Check state (ChargedBack deposits are evicted, so not found)
        if record.state == DepositState::Disputed {
            return Err(DepositOperationError::InvalidState(Dispute, tx));
        }

        let amount = record.amount;
        record.state = DepositState::Disputed; // Update state in place (no second lookup)

        let account = self
            .clients
            .get_mut(&client)
            .ok_or(DepositOperationError::ClientNotFound(Dispute, client))?;

        // Move funds from available to held (may result in negative available balance)
        if account.available() < amount {
            warn!(
                client = client,
                available = %account.available(),
                required = %amount,
                "dispute will cause negative available balance"
            );
        }
        account.hold(amount);

        Ok(())
    }

    /// Apply a `Transaction::Resolve`:
    /// - Find the referenced deposit
    /// - Validate client ownership
    /// - Check deposit is in Disputed state
    /// - Move funds from held back to available
    fn apply_resolve(&mut self, client: ClientId, tx: TxId) -> Result<(), DepositOperationError> {
        use DepositOperation::Resolve;

        let record = self
            .deposits
            .get_mut(&tx)
            .ok_or(DepositOperationError::TxNotFound(Resolve, tx))?;

        // Validate client ownership
        if record.client != client {
            return Err(DepositOperationError::ClientMismatch(
                Resolve,
                tx,
                record.client,
                client,
            ));
        }

        // Check state (ChargedBack deposits are evicted, so not found)
        if record.state == DepositState::Ok {
            return Err(DepositOperationError::InvalidState(Resolve, tx));
        }

        let amount = record.amount;
        record.state = DepositState::Ok; // Update state in place (no second lookup)

        let account = self
            .clients
            .get_mut(&client)
            .ok_or(DepositOperationError::ClientNotFound(Resolve, client))?;

        // Move held back to available
        account.release(amount);

        Ok(())
    }

    /// Apply a `Transaction::Chargeback`:
    /// - Find the referenced deposit
    /// - Validate client ownership
    /// - Check deposit is in Disputed state
    /// - Remove held funds (total decreases), freeze account
    /// - Evict deposit (terminal state, can never be referenced again)
    fn apply_chargeback(
        &mut self,
        client: ClientId,
        tx: TxId,
    ) -> Result<(), DepositOperationError> {
        use DepositOperation::Chargeback;

        let record = self
            .deposits
            .get(&tx)
            .ok_or(DepositOperationError::TxNotFound(Chargeback, tx))?;

        // Validate client ownership
        if record.client != client {
            return Err(DepositOperationError::ClientMismatch(
                Chargeback,
                tx,
                record.client,
                client,
            ));
        }

        // Check state (ChargedBack deposits are evicted, so not found)
        if record.state == DepositState::Ok {
            return Err(DepositOperationError::InvalidState(Chargeback, tx));
        }

        let amount = record.amount;

        let account = self
            .clients
            .get_mut(&client)
            .ok_or(DepositOperationError::ClientNotFound(Chargeback, client))?;

        // Remove held funds (total decreases)
        account.remove_held(amount);

        // Freeze account and evict deposit (terminal state)
        account.freeze();
        self.deposits.remove(&tx);

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
        assert_eq!(client.available(), Amount::from_scaled(100));
        assert_eq!(client.held(), Amount::from_scaled(0));
        assert!(!client.is_frozen());
    }

    #[test]
    fn deposit_accumulates_balance() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(deposit(1, 2, 50)).unwrap();

        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available(), Amount::from_scaled(150));
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
        assert_eq!(client.available(), Amount::from_scaled(100));
    }

    // Withdrawal

    #[test]
    fn withdrawal_decreases_balance() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(withdrawal(1, 2, 30)).unwrap();

        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available(), Amount::from_scaled(70));
    }

    #[test]
    fn withdrawal_exact_amount_succeeds() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(withdrawal(1, 2, 100)).unwrap();

        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available(), Amount::from_scaled(0));
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
        assert_eq!(client.available(), Amount::from_scaled(100));
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
        assert_eq!(client.available(), Amount::from_scaled(100));
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

        assert_eq!(client1.available(), Amount::from_scaled(70));
        assert_eq!(client2.available(), Amount::from_scaled(200));
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
        let c1 = clients.iter().find(|c| c.id() == 1).unwrap();
        let c2 = clients.iter().find(|c| c.id() == 2).unwrap();

        assert_eq!(c1.available(), Amount::from_scaled(100));
        assert_eq!(c2.available(), Amount::from_scaled(200));
    }

    //  Async run()

    #[tokio::test]
    async fn run_processes_all_transactions() {
        let mut engine = Engine::new();
        let transactions = vec![deposit(1, 1, 100), deposit(2, 2, 200), withdrawal(1, 3, 25)];

        engine.run(tokio_stream::iter(transactions)).await;

        let client1 = engine.get_client(1).unwrap();
        let client2 = engine.get_client(2).unwrap();

        assert_eq!(client1.available(), Amount::from_scaled(75));
        assert_eq!(client2.available(), Amount::from_scaled(200));
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

        assert_eq!(client.available(), Amount::from_scaled(150)); // 100 + 50 with withdrawal skipped
    }

    // Dispute, Resolve, Chargeback - test utils

    fn dispute(client: ClientId, tx: TxId) -> Transaction {
        Transaction::Dispute { client, tx }
    }

    fn resolve(client: ClientId, tx: TxId) -> Transaction {
        Transaction::Resolve { client, tx }
    }

    fn chargeback(client: ClientId, tx: TxId) -> Transaction {
        Transaction::Chargeback { client, tx }
    }

    // Dispute tests

    #[test]
    fn dispute_deposit_moves_funds_to_held() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(dispute(1, 1)).unwrap();

        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available(), Amount::from_scaled(0));
        assert_eq!(client.held(), Amount::from_scaled(100));
        assert_eq!(client.total(), Amount::from_scaled(100));
        assert!(!client.is_frozen());
    }

    #[test]
    fn dispute_withdrawal_fails() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(withdrawal(1, 2, 40)).unwrap();

        // Withdrawals can't be disputed - they're not in the deposits map
        let result = engine.apply(dispute(1, 2));
        assert!(matches!(
            result,
            Err(EngineError::DepositOperation(
                DepositOperationError::TxNotFound(DepositOperation::Dispute, 2)
            ))
        ));

        // Balance unchanged
        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available(), Amount::from_scaled(60));
        assert_eq!(client.held(), Amount::from_scaled(0));
    }

    #[test]
    fn dispute_nonexistent_tx_fails() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();

        let result = engine.apply(dispute(1, 999));
        assert!(matches!(
            result,
            Err(EngineError::DepositOperation(
                DepositOperationError::TxNotFound(DepositOperation::Dispute, 999)
            ))
        ));
    }

    #[test]
    fn dispute_wrong_client_fails() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(deposit(2, 2, 50)).unwrap();

        let result = engine.apply(dispute(2, 1)); // tx 1 belongs to client 1
        assert!(matches!(
            result,
            Err(EngineError::DepositOperation(
                DepositOperationError::ClientMismatch(DepositOperation::Dispute, 1, 1, 2)
            ))
        ));
    }

    #[test]
    fn dispute_already_disputed_fails() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(dispute(1, 1)).unwrap();

        let result = engine.apply(dispute(1, 1));
        assert!(matches!(
            result,
            Err(EngineError::DepositOperation(
                DepositOperationError::InvalidState(DepositOperation::Dispute, 1)
            ))
        ));
    }

    #[test]
    fn dispute_deposit_insufficient_funds_causes_negative_balance() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(withdrawal(1, 2, 60)).unwrap(); // leaves 40 available

        // Dispute succeeds even with insufficient funds (logs warning)
        engine.apply(dispute(1, 1)).unwrap(); // needs 100, has 40

        // Available is now negative (-60), held is 100
        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available(), Amount::from_scaled(-60));
        assert_eq!(client.held(), Amount::from_scaled(100));
        assert_eq!(client.total(), Amount::from_scaled(40)); // total unchanged
    }

    // Resolve tests

    #[test]
    fn resolve_deposit_returns_funds_to_available() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(dispute(1, 1)).unwrap();
        engine.apply(resolve(1, 1)).unwrap();

        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available(), Amount::from_scaled(100));
        assert_eq!(client.held(), Amount::from_scaled(0));
        assert!(!client.is_frozen());
    }

    #[test]
    fn resolve_not_disputed_fails() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();

        let result = engine.apply(resolve(1, 1));
        assert!(matches!(
            result,
            Err(EngineError::DepositOperation(
                DepositOperationError::InvalidState(DepositOperation::Resolve, 1)
            ))
        ));
    }

    #[test]
    fn resolved_tx_can_be_disputed_again() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(dispute(1, 1)).unwrap();
        engine.apply(resolve(1, 1)).unwrap();
        engine.apply(dispute(1, 1)).unwrap(); // should succeed

        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available(), Amount::from_scaled(0));
        assert_eq!(client.held(), Amount::from_scaled(100));
    }

    // Chargeback tests

    #[test]
    fn chargeback_deposit_removes_held_and_freezes() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(dispute(1, 1)).unwrap();
        engine.apply(chargeback(1, 1)).unwrap();

        let client = engine.get_client(1).unwrap();
        assert_eq!(client.available(), Amount::from_scaled(0));
        assert_eq!(client.held(), Amount::from_scaled(0));
        assert_eq!(client.total(), Amount::from_scaled(0));
        assert!(client.is_frozen());
    }

    #[test]
    fn chargeback_not_disputed_fails() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();

        let result = engine.apply(chargeback(1, 1));
        assert!(matches!(
            result,
            Err(EngineError::DepositOperation(
                DepositOperationError::InvalidState(DepositOperation::Chargeback, 1)
            ))
        ));
    }

    #[test]
    fn chargedback_tx_cannot_be_disputed() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(dispute(1, 1)).unwrap();
        engine.apply(chargeback(1, 1)).unwrap();

        // Chargedback transactions are evicted, so they appear as "not found"
        let result = engine.apply(dispute(1, 1));
        assert!(matches!(
            result,
            Err(EngineError::DepositOperation(
                DepositOperationError::TxNotFound(DepositOperation::Dispute, 1)
            ))
        ));
    }

    // Duplicate transaction ID tests

    #[test]
    fn duplicate_deposit_tx_id_fails() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();

        let result = engine.apply(deposit(1, 1, 50));
        assert!(matches!(
            result,
            Err(EngineError::Deposit(DepositError::DuplicateTxId(1)))
        ));
    }

    #[test]
    fn duplicate_withdrawal_tx_id_fails() {
        let mut engine = Engine::new();
        engine.apply(deposit(1, 1, 100)).unwrap();
        engine.apply(withdrawal(1, 2, 30)).unwrap();

        let result = engine.apply(withdrawal(1, 2, 20));
        assert!(matches!(
            result,
            Err(EngineError::Withdrawal(WithdrawalError::DuplicateTxId(2)))
        ));
    }
}
