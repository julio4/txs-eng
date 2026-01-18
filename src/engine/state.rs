//! Client account state.

use crate::Amount;
use crate::model::ClientId;

/// A client's account with available and held balances.
///
/// Accounts can be frozen/locked after a chargeback, preventing further transactions.
#[derive(Debug)]
pub struct ClientAccount {
    /// The client identifier.
    id: ClientId,
    /// Funds available for withdrawal/trading.
    available: Amount,
    /// Funds held due to a dispute.
    held: Amount,
    /// Whether the account is frozen (no deposits or withdrawals allowed).
    frozen: bool,
}

impl ClientAccount {
    /// Create a new account for the given client.
    pub fn new(id: ClientId) -> Self {
        Self {
            id,
            available: Amount::default(),
            held: Amount::default(),
            frozen: false,
        }
    }

    // Getters

    /// Returns the client identifier.
    pub fn id(&self) -> ClientId {
        self.id
    }

    /// Returns the available balance.
    pub fn available(&self) -> Amount {
        self.available
    }

    /// Returns the held balance.
    pub fn held(&self) -> Amount {
        self.held
    }

    /// Returns whether the account is frozen.
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    /// Total funds (available + held).
    pub fn total(&self) -> Amount {
        self.available + self.held
    }

    // Mutations

    /// Credit funds to available balance.
    pub fn credit(&mut self, amount: Amount) {
        self.available += amount;
    }

    /// Debit funds from available balance.
    pub fn debit(&mut self, amount: Amount) {
        self.available -= amount;
    }

    /// Hold funds: move from available to held.
    pub fn hold(&mut self, amount: Amount) {
        self.available -= amount;
        self.held += amount;
    }

    /// Release funds: move from held back to available.
    pub fn release(&mut self, amount: Amount) {
        self.held -= amount;
        self.available += amount;
    }

    /// Remove held funds (for chargeback).
    pub fn remove_held(&mut self, amount: Amount) {
        self.held -= amount;
    }

    /// Freeze the account, preventing further transactions.
    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    /// Unfreeze the account (for admin).
    pub fn unfreeze(&mut self) {
        self.frozen = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_account_new() {
        let account = ClientAccount::new(42);
        assert_eq!(account.id(), 42);
        assert_eq!(account.available(), Amount::default());
        assert_eq!(account.held(), Amount::default());
        assert!(!account.is_frozen());
    }

    #[test]
    fn client_account_total_sums_available_and_held() {
        let mut account = ClientAccount::new(1);
        account.credit(Amount::from_scaled(100));
        account.hold(Amount::from_scaled(50));
        // available is now 50, held is 50
        assert_eq!(account.total(), Amount::from_scaled(100));
    }

    #[test]
    fn credit_and_debit() {
        let mut account = ClientAccount::new(1);
        account.credit(Amount::from_scaled(100));
        assert_eq!(account.available(), Amount::from_scaled(100));
        account.debit(Amount::from_scaled(30));
        assert_eq!(account.available(), Amount::from_scaled(70));
    }

    #[test]
    fn hold_and_release() {
        let mut account = ClientAccount::new(1);
        account.credit(Amount::from_scaled(100));
        account.hold(Amount::from_scaled(40));
        assert_eq!(account.available(), Amount::from_scaled(60));
        assert_eq!(account.held(), Amount::from_scaled(40));

        account.release(Amount::from_scaled(40));
        assert_eq!(account.available(), Amount::from_scaled(100));
        assert_eq!(account.held(), Amount::from_scaled(0));
    }

    #[test]
    fn remove_held() {
        let mut account = ClientAccount::new(1);
        account.credit(Amount::from_scaled(100));
        account.hold(Amount::from_scaled(100));
        account.remove_held(Amount::from_scaled(100));
        assert_eq!(account.held(), Amount::from_scaled(0));
        assert_eq!(account.available(), Amount::from_scaled(0));
    }

    #[test]
    fn freeze_and_unfreeze() {
        let mut account = ClientAccount::new(1);
        assert!(!account.is_frozen());
        account.freeze();
        assert!(account.is_frozen());
        account.unfreeze();
        assert!(!account.is_frozen());
    }
}
