//! Error types for transaction processing.

use thiserror::Error;

use crate::Amount;
use crate::model::{ClientId, TxId};

/// Top-level error returned by [`Engine::apply`](super::Engine::apply).
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("deposit failed: {0}")]
    Deposit(#[from] DepositError),

    #[error("withdrawal failed: {0}")]
    Withdrawal(#[from] WithdrawalError),

    #[error("{0}")]
    DepositOperation(#[from] DepositOperationError),
}

/// Error during deposit processing.
#[derive(Debug, Error)]
pub enum DepositError {
    #[error("account {0} is frozen")]
    AccountFrozen(ClientId),
    #[error("duplicate transaction id {0}")]
    DuplicateTxId(TxId),
}

/// Error during withdrawal processing.
#[derive(Debug, Error)]
pub enum WithdrawalError {
    #[error("account {0} is frozen")]
    AccountFrozen(ClientId),
    #[error("insufficient available funds for client {0}: available {1}, requested {2}")]
    InsufficientFunds(ClientId, Amount, Amount),
    #[error("duplicate transaction id {0}")]
    DuplicateTxId(TxId),
}

/// The type of deposit operation being performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepositOperation {
    Dispute,
    Resolve,
    Chargeback,
}

/// Unified error for deposit operations (dispute, resolve, chargeback).
#[derive(Debug, Error)]
pub enum DepositOperationError {
    #[error("{0:?}: deposit {1} not found")]
    TxNotFound(DepositOperation, TxId),

    #[error("{0:?}: deposit {1} belongs to client {2}, not {3}")]
    ClientMismatch(DepositOperation, TxId, ClientId, ClientId),

    #[error("{0:?}: deposit {1} is not in expected state")]
    InvalidState(DepositOperation, TxId),

    #[error("{0:?}: client {1} not found")]
    ClientNotFound(DepositOperation, ClientId),
}
