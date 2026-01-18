use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use txs_eng::{Amount, ClientId, Engine, Transaction, TxId};

/// Generates valid transaction sequences for benchmarking.
///
/// Pattern per client (repeating):
/// 1. Deposit 100
/// 2. Deposit 50
/// 3. Withdrawal 30
///
/// This ensures withdrawals never exceed available funds.
pub struct TxGenerator {
    next_tx_id: TxId,
    num_clients: ClientId,
    txs_per_client: u32,
    current_client: ClientId,
    current_step: u32,
}

impl TxGenerator {
    pub fn new(num_clients: ClientId, txs_per_client: u32) -> Self {
        Self {
            next_tx_id: 1,
            num_clients,
            txs_per_client,
            current_client: 1,
            current_step: 0,
        }
    }

    /// Total number of transactions this generator will produce
    pub fn total_transactions(&self) -> u64 {
        self.num_clients as u64 * self.txs_per_client as u64
    }
}

impl Iterator for TxGenerator {
    type Item = Transaction;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_client > self.num_clients {
            return None;
        }

        let tx_id = self.next_tx_id;
        self.next_tx_id += 1;

        // Pattern: deposit 100, deposit 50, withdrawal 30 (repeating)
        let tx = match self.current_step % 3 {
            0 => Transaction::Deposit {
                client: self.current_client,
                tx: tx_id,
                amount: Amount::from_scaled(1000), // 100.0
            },
            1 => Transaction::Deposit {
                client: self.current_client,
                tx: tx_id,
                amount: Amount::from_scaled(500), // 50.0
            },
            _ => Transaction::Withdrawal {
                client: self.current_client,
                tx: tx_id,
                amount: Amount::from_scaled(300), // 30.0
            },
        };

        self.current_step += 1;

        // Move to next client after txs_per_client transactions
        if self.current_step >= self.txs_per_client {
            self.current_step = 0;
            self.current_client += 1;
        }

        Some(tx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let total = self.total_transactions() as usize;
        let done = (self.current_client.saturating_sub(1) as u64 * self.txs_per_client as u64
            + self.current_step as u64) as usize;
        let remaining = total.saturating_sub(done);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for TxGenerator {}

/// Generator with disputes interspersed
pub struct TxGeneratorWithDisputes {
    inner: TxGenerator,
    /// Dispute every Nth deposit (0 = no disputes)
    dispute_every: u32,
    deposits_since_dispute: u32,
    /// Queue of pending dispute transactions
    pending_disputes: Vec<Transaction>,
    /// Track deposit tx IDs for disputes
    recent_deposits: Vec<(ClientId, TxId)>,
}

impl TxGeneratorWithDisputes {
    pub fn new(num_clients: ClientId, txs_per_client: u32, dispute_every: u32) -> Self {
        Self {
            inner: TxGenerator::new(num_clients, txs_per_client),
            dispute_every,
            deposits_since_dispute: 0,
            pending_disputes: Vec::new(),
            recent_deposits: Vec::new(),
        }
    }
}

impl Iterator for TxGeneratorWithDisputes {
    type Item = Transaction;

    fn next(&mut self) -> Option<Self::Item> {
        // First, drain any pending disputes
        if let Some(dispute) = self.pending_disputes.pop() {
            return Some(dispute);
        }

        let tx = self.inner.next()?;

        // Track deposits for potential disputes
        if let Transaction::Deposit {
            client, tx: tx_id, ..
        } = &tx
        {
            self.recent_deposits.push((*client, *tx_id));
            self.deposits_since_dispute += 1;

            // Time to dispute?
            if self.dispute_every > 0 && self.deposits_since_dispute >= self.dispute_every {
                self.deposits_since_dispute = 0;
                // Dispute a recent deposit (not the current one, to ensure it's applied first)
                if self.recent_deposits.len() > 1 {
                    let idx = self.recent_deposits.len() / 2;
                    let (client, tx_id) = self.recent_deposits.remove(idx);
                    self.pending_disputes
                        .push(Transaction::Dispute { client, tx: tx_id });
                }
            }
        }

        Some(tx)
    }
}

fn bench_deposit_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("deposits");

    for count in [10_000u32, 100_000, 1_000_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let mut engine = Engine::new();
                let generator = TxGenerator::new(1, count);
                for tx in generator {
                    let _ = black_box(engine.apply(tx));
                }
                engine
            });
        });
    }

    group.finish();
}

fn bench_mixed_transactions(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed");

    // Multiple clients with mixed transactions
    for (clients, txs_per) in [(100, 1_000), (1_000, 100), (10, 10_000)] {
        let label = format!("{}c_{}tx", clients, txs_per);
        group.bench_with_input(
            BenchmarkId::from_parameter(&label),
            &(clients, txs_per),
            |b, &(clients, txs_per)| {
                b.iter(|| {
                    let mut engine = Engine::new();
                    let generator = TxGenerator::new(clients, txs_per);
                    for tx in generator {
                        let _ = black_box(engine.apply(tx));
                    }
                    engine
                });
            },
        );
    }

    group.finish();
}

fn bench_with_disputes(c: &mut Criterion) {
    let mut group = c.benchmark_group("with_disputes");

    // 100k transactions with disputes every 100 deposits
    group.bench_function("100k_dispute_1pct", |b| {
        b.iter(|| {
            let mut engine = Engine::new();
            let generator = TxGeneratorWithDisputes::new(100, 1_000, 100);
            for tx in generator {
                let _ = black_box(engine.apply(tx));
            }
            engine
        });
    });

    group.finish();
}

fn bench_large_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_scale");
    group.sample_size(10); // Fewer samples for large benchmarks

    // > u16::MAX transactions
    let count = 70_000u32; // Just over u16::MAX (65,535)
    group.bench_function("70k_single_client", |b| {
        b.iter(|| {
            let mut engine = Engine::new();
            let generator = TxGenerator::new(1, count);
            for tx in generator {
                let _ = black_box(engine.apply(tx));
            }
            engine
        });
    });

    // Multiple clients, total > u16::MAX
    group.bench_function("100k_multi_client", |b| {
        b.iter(|| {
            let mut engine = Engine::new();
            let generator = TxGenerator::new(100, 1_000);
            for tx in generator {
                let _ = black_box(engine.apply(tx));
            }
            engine
        });
    });

    group.finish();
}

fn bench_stress_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress_test");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(60));

    // 100M transactions
    group.bench_function("100M", |b| {
        b.iter(|| {
            let mut engine = Engine::new();
            let generator = TxGenerator::new(1000, 100_000);
            for tx in generator {
                let _ = black_box(engine.apply(tx));
            }
            engine
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_deposit_only,
    bench_mixed_transactions,
    bench_with_disputes,
    bench_large_scale,
);

criterion_group!(
    name = stress;
    config = Criterion::default().sample_size(10);
    targets = bench_stress_test
);

criterion_main!(benches, stress);
