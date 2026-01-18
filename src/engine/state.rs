//! Client account state.

use crate::Amount;

/// A client's account with available and held balances.
///
/// Accounts can be frozen/locked after a chargeback, preventing further transactions.
#[derive(Debug, Default)]
pub struct ClientAccount {
    /// Funds available for withdrawal/trading.
    pub available: Amount,
    /// Funds held due to a dispute.
    pub held: Amount,
    /// Whether the account is frozen (no deposits or withdrawals allowed).
    pub frozen: bool,
}

impl ClientAccount {
    /// Total funds (available + held).
    pub fn total(&self) -> Amount {
        self.available + self.held
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
    fn client_account_default() {
        let account = ClientAccount::default();
        assert_eq!(account.available, Amount::default());
        assert_eq!(account.held, Amount::default());
        assert!(!account.frozen);
    }

    #[test]
    fn client_account_total_sums_available_and_held() {
        let account = ClientAccount {
            available: Amount::from_scaled(100),
            held: Amount::from_scaled(50),
            frozen: false,
        };
        assert_eq!(account.total(), Amount::from_scaled(150));
    }
}
