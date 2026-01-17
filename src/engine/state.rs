use crate::Amount;

/// A client account with its available and held balance.
/// An account can also be frozen.
#[derive(Debug, Default)]
pub struct ClientAccount {
    pub available: Amount,
    pub held: Amount,
    pub frozen: bool,
}

impl ClientAccount {
    pub fn total(&self) -> Amount {
        self.available + self.held
    }

    pub fn freeze(&mut self) {
        self.frozen = true;
    }

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
