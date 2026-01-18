//! Core domain types for the transaction engine.

use crate::Amount;

/// Client identifier.
pub type ClientId = u16;

/// Transaction identifier.
pub type TxId = u32;

/// A transaction representing the possible inputs of the engine.
#[derive(Debug, Clone)]
pub enum Transaction {
    /// Credit funds to a client's available balance.
    Deposit {
        client: ClientId,
        tx: TxId,
        amount: Amount,
    },
    /// Debit funds from a client's available balance.
    Withdrawal {
        client: ClientId,
        tx: TxId,
        amount: Amount,
    },
    /// Claim a deposit was erroneous; moves corresponding funds from available to held.
    Dispute { client: ClientId, tx: TxId },
    /// Release disputed funds back to available.
    Resolve { client: ClientId, tx: TxId },
    /// Reverse a disputed deposit; removes held funds and freezes account.
    Chargeback { client: ClientId, tx: TxId },
}

/// State of a deposit for dispute tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DepositState {
    /// Deposit is valid and can be disputed.
    #[default]
    Ok,
    /// Deposit is currently under dispute.
    Disputed,
    // Chargeback is a final state
}

/// Record of a deposit for dispute tracking.
#[derive(Debug, Clone)]
pub struct DepositRecord {
    /// The client who made the deposit.
    pub client: ClientId,
    /// The deposited amount.
    pub amount: Amount,
    /// Current dispute state.
    pub state: DepositState,
}

impl DepositRecord {
    /// Create a new deposit record in the `Ok` state.
    pub fn new(client: ClientId, amount: Amount) -> Self {
        Self {
            client,
            amount,
            state: DepositState::Ok,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_record_size() {
        // DepositRecord layout:
        // - amount: 8 bytes
        // - client: 2 bytes
        // - state: 1 byte
        // - padding: 5 bytes
        assert_eq!(std::mem::size_of::<DepositRecord>(), 16);
    }

    #[test]
    fn deposit_state_default() {
        assert_eq!(DepositState::default(), DepositState::Ok);
    }
}
