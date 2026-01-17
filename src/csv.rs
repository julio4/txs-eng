use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;
use thiserror::Error;

use crate::{Amount, ClientId, Transaction, TxId};

/// Errors that can occur when parsing csv rows
#[derive(Debug, Error)]
pub enum CsvError {
    #[error("line {line}: failed to parse row: {source}")]
    Parse { line: usize, source: csv::Error },

    #[error("line {line}: unrecognized transaction type '{tx_type}'")]
    UnrecognizedType { line: usize, tx_type: String },

    #[error("line {line}: {tx_type} missing amount")]
    MissingAmount { line: usize, tx_type: String },
}

#[derive(Debug, Deserialize)]
struct InputRow {
    r#type: String,
    client: ClientId,
    tx: TxId,
    amount: Option<f64>,
}

#[derive(Debug, Serialize)]
struct OutputRow {
    client: ClientId,
    available: String,
    held: String,
    total: String,
    locked: bool,
}

/// Read transactions from a csv file
pub fn read_transactions(
    path: impl AsRef<Path>,
) -> impl Iterator<Item = Result<Transaction, CsvError>> {
    let reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(path)
        .expect("failed to open csv file");

    reader
        .into_deserialize::<InputRow>()
        .enumerate()
        .map(|(idx, result)| {
            let line = idx + 2; // 1-indexed, skip header
            let row = result.map_err(|source| CsvError::Parse { line, source })?;
            match row.r#type.as_str() {
                "deposit" => {
                    let amount = row.amount.ok_or_else(|| CsvError::MissingAmount {
                        line,
                        tx_type: "deposit".to_string(),
                    })?;
                    Ok(Transaction::Deposit {
                        client: row.client,
                        tx: row.tx,
                        amount: Amount::from_float(amount),
                    })
                }
                "withdrawal" => {
                    let amount = row.amount.ok_or_else(|| CsvError::MissingAmount {
                        line,
                        tx_type: "withdrawal".to_string(),
                    })?;
                    Ok(Transaction::Withdrawal {
                        client: row.client,
                        tx: row.tx,
                        amount: Amount::from_float(amount),
                    })
                }
                other => Err(CsvError::UnrecognizedType {
                    line,
                    tx_type: other.to_string(),
                }),
            }
        })
}

/// write client accounts to stdout in csv format
pub fn write_accounts(
    accounts: impl IntoIterator<Item = (ClientId, Amount, Amount, Amount, bool)>,
) {
    let stdout = io::stdout();
    let mut writer = csv::Writer::from_writer(stdout.lock());

    for (client, available, held, total, locked) in accounts {
        let row = OutputRow {
            client,
            available: available.to_string(),
            held: held.to_string(),
            total: total.to_string(),
            locked,
        };
        writer.serialize(&row).expect("failed to write csv row");
    }

    writer.flush().expect("failed to flush csv writer");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_csv(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn read_deposit() {
        let file = write_csv("type,client,tx,amount\ndeposit,1,1,10.5\n");
        let results: Vec<_> = read_transactions(file.path()).collect();
        assert_eq!(results.len(), 1);

        let tx = results.into_iter().next().unwrap().unwrap();
        match tx {
            Transaction::Deposit { client, tx, amount } => {
                assert_eq!(client, 1);
                assert_eq!(tx, 1);
                assert_eq!(amount, Amount::from_float(10.5));
            }
            _ => panic!("expected deposit"),
        }
    }

    #[test]
    fn read_withdrawal() {
        let file = write_csv("type,client,tx,amount\nwithdrawal,2,3,5.25\n");
        let results: Vec<_> = read_transactions(file.path()).collect();
        assert_eq!(results.len(), 1);

        let tx = results.into_iter().next().unwrap().unwrap();
        match tx {
            Transaction::Withdrawal { client, tx, amount } => {
                assert_eq!(client, 2);
                assert_eq!(tx, 3);
                assert_eq!(amount, Amount::from_float(5.25));
            }
            _ => panic!("expected withdrawal"),
        }
    }

    #[test]
    fn read_with_whitespace() {
        let file = write_csv("type, client, tx, amount\ndeposit, 1, 1, 10.0\n");
        let results: Vec<_> = read_transactions(file.path()).collect();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }

    #[test]
    fn read_returns_error_for_unknown_type() {
        let file = write_csv("type,client,tx,amount\nunknown,1,1,10.0\n");
        let results: Vec<_> = read_transactions(file.path()).collect();
        assert_eq!(results.len(), 1);
        let err = results[0].as_ref().unwrap_err();
        assert!(matches!(err, CsvError::UnrecognizedType { line: 2, .. }));
    }

    #[test]
    fn read_returns_error_for_missing_amount() {
        let file = write_csv("type,client,tx,amount\ndeposit,1,1,\n");
        let results: Vec<_> = read_transactions(file.path()).collect();
        assert_eq!(results.len(), 1);
        let err = results[0].as_ref().unwrap_err();
        assert!(matches!(err, CsvError::MissingAmount { line: 2, .. }));
    }
}
