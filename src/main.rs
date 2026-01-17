use std::env;

use tokio_stream::wrappers::ReceiverStream;
use tracing::warn;
use tracing_subscriber::EnvFilter;
use txs_eng::Engine;
use txs_eng::csv::{read_transactions, write_accounts};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("warn".parse().unwrap()))
        .with_writer(std::io::stderr)
        .init();

    let path = env::args()
        .nth(1)
        .expect("usage: txs-eng <transactions.csv>");

    if !path.ends_with(".csv") {
        warn!(path, "input file seems to not be a csv file");
    }

    let mut engine = Engine::new();
    let (tx_sender, tx_receiver) = tokio::sync::mpsc::channel(16);

    tokio::spawn(async move {
        for result in read_transactions(&path) {
            match result {
                Ok(tx) => {
                    tx_sender.send(tx).await.unwrap();
                }
                Err(e) => {
                    warn!("{e}");
                }
            }
        }
    });

    engine.run(ReceiverStream::new(tx_receiver)).await;

    write_accounts(engine.clients());
}
