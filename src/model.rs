use crate::Amount;

/// Client identifier
pub type ClientId = u16;

/// Transaction identifier
pub type TxId = u16;

/// A transaction representing a user's intent.
#[derive(Debug, Clone)]
pub enum Transaction {
    Deposit {
        client: ClientId,
        tx: TxId,
        amount: Amount,
    },
    Withdrawal {
        client: ClientId,
        tx: TxId,
        amount: Amount,
    },
}
