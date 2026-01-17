use tokio_stream::wrappers::ReceiverStream;
use tracing_subscriber::EnvFilter;
use txs_eng::{Amount, Engine, Transaction};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("txs_eng=warning".parse().unwrap()),
        )
        .init();

    let mut engine = Engine::new();
    let (tx_sender, tx_receiver) = tokio::sync::mpsc::channel(16);

    tokio::spawn(async move {
        let transactions = [
            Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: Amount::from_scaled(100_0000),
            },
            Transaction::Deposit {
                client: 2,
                tx: 2,
                amount: Amount::from_scaled(50_0000),
            },
            Transaction::Withdrawal {
                client: 1,
                tx: 3,
                amount: Amount::from_scaled(25_0000),
            },
            Transaction::Withdrawal {
                client: 1,
                tx: 4,
                amount: Amount::from_scaled(200_0000),
            },
        ];

        for tx in transactions {
            tx_sender.send(tx).await.unwrap();
        }
    });

    engine.run(ReceiverStream::new(tx_receiver)).await;

    // debug view for now, lets focus on csv export after
    println!("client,available,held,total,locked");
    for (client, available, held, total, locked) in engine.clients() {
        println!("{client},{available},{held},{total},{locked}");
    }
}
