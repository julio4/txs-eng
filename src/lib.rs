//! A simple transaction processing engine for handling deposits and withdrawals with dispute support.
//!
//! # Usage
//!
//! ```no_run
//! use txs_eng::{Engine, Transaction, Amount};
//!
//! let mut engine = Engine::new();
//! engine.apply(Transaction::Deposit {
//!     client: 1,
//!     tx: 1,
//!     amount: Amount::from_float(100.0),
//! });
//! ```

pub mod amount;
pub mod csv;
pub mod engine;
pub mod model;

pub use amount::Amount;
pub use engine::Engine;
pub use model::{ClientId, Transaction, TxId};
