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

    #[error("dispute failed: {0}")]
    Dispute(#[from] DisputeError),

    #[error("resolve failed: {0}")]
    Resolve(#[from] ResolveError),

    #[error("chargeback failed: {0}")]
    Chargeback(#[from] ChargebackError),
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

/// Error during dispute processing.
#[derive(Debug, Error)]
pub enum DisputeError {
    #[error("deposit {0} not found")]
    TxNotFound(TxId),
    #[error("client mismatch: deposit {0} belongs to client {1}, not {2}")]
    ClientMismatch(TxId, ClientId, ClientId),
    #[error("deposit {0} already disputed")]
    AlreadyDisputed(TxId),
    /// Internal error
    #[error("client {0} not found")]
    ClientNotFound(ClientId),
}

/// Error during resolve processing.
#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("deposit {0} not found")]
    TxNotFound(TxId),
    #[error("client mismatch: deposit {0} belongs to client {1}, not {2}")]
    ClientMismatch(TxId, ClientId, ClientId),
    #[error("deposit {0} is not disputed")]
    NotDisputed(TxId),
    /// Internal error
    #[error("client {0} not found")]
    ClientNotFound(ClientId),
}

/// Error during chargeback processing.
#[derive(Debug, Error)]
pub enum ChargebackError {
    #[error("deposit {0} not found")]
    TxNotFound(TxId),
    #[error("client mismatch: deposit {0} belongs to client {1}, not {2}")]
    ClientMismatch(TxId, ClientId, ClientId),
    #[error("deposit {0} is not disputed")]
    NotDisputed(TxId),
    /// Internal error
    #[error("client {0} not found")]
    ClientNotFound(ClientId),
}
