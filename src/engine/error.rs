use thiserror::Error;

use crate::Amount;
use crate::model::ClientId;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("deposit failed: {0}")]
    Deposit(#[from] DepositError),

    #[error("withdrawal failed: {0}")]
    Withdrawal(#[from] WithdrawalError),
}

#[derive(Debug, Error)]
pub enum DepositError {
    #[error("account {0} is frozen")]
    AccountFrozen(ClientId),
}

#[derive(Debug, Error)]
pub enum WithdrawalError {
    #[error("account {0} is frozen")]
    AccountFrozen(ClientId),

    #[error("insufficient available funds for client {0}: available {1}, requested {2}")]
    InsufficientFunds(ClientId, Amount, Amount),
}
