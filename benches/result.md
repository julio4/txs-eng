# Benchmark Results

## Memory Usage

| Transactions | RAM (TxRecord storage) |
|-------------:|----------------------:|
| 1M           | 16 MB                 |
| 10M          | 160 MB                |
| 100M         | 1.6 GB                |
| 1B           | 16 GB                 |
| u32::MAX     | 68 GB                 |

*TxRecord = 16 bytes (compact: 8B amount + 2B client + 1B meta + 5B padding)*

## Throughput

| Benchmark | Transactions | Time | Throughput |
|-----------|-------------:|-----:|-----------:|
| deposits/10k | 10,000 | 753 µs | 13M tx/s |
| deposits/100k | 100,000 | 8.4 ms | 12M tx/s |
| deposits/1M | 1,000,000 | 84 ms | 12M tx/s |
| mixed/100c_1000tx | 100,000 | 8.5 ms | 12M tx/s |
| mixed/1000c_100tx | 100,000 | 8.8 ms | 11M tx/s |
| mixed/10c_10000tx | 100,000 | 8.7 ms | 11M tx/s |
| with_disputes/100k_1pct | 100,000 | 11 ms | 9M tx/s |
| large_scale/70k_single | 70,000 | 5.4 ms | 13M tx/s |
| large_scale/100k_multi | 100,000 | 8.5 ms | 12M tx/s |
| stress_test/100M | 100,000,000 | 33 s | 3M tx/s |

Throughput decreases at scale due to HashMap growth and cache pressure.

## Usage

```bash
cargo bench -- large_scale       # 70k-100k transactions
cargo bench -- stress_test       # 100M transactions
```
