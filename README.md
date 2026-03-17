# Payments Engine

A toy payments engine that reads transactions from CSV, processes deposits, withdrawals, disputes, resolves, and chargebacks, then outputs client account states to stdout.

## Usage

```sh
cargo run -- transactions.csv > accounts.csv
```

## Design

Three modules, each with a single responsibility:

- **`types.rs`** — Data types for input (`InputRecord`), output (`OutputRecord`), engine state (`Account`, `StoredTransaction`), and the `TransactionType` enum.
- **`engine.rs`** — `PaymentsEngine` processes transactions and holds all state: a `HashMap<u16, Account>` for clients and `HashMap<u32, StoredTransaction>` for dispute lookups.
- **`main.rs`** — CLI entry point. Streams CSV in via `csv::Reader`, feeds records to the engine, writes results via `csv::Writer`.

### Key Decisions

**`rust_decimal::Decimal` instead of `f64`** — Exact decimal arithmetic. No floating-point rounding errors with money (`0.1 + 0.2 == 0.3`).

**Streaming** — Input is read record-by-record through a `BufReader` (8KB chunks). Memory usage is constant regardless of file size. Only deposits are stored (for dispute lookups); withdrawals are not retained.

**Dispute state machine** — Each stored transaction tracks `disputed` and `chargebacked` flags:

```
                dispute          resolve
  [normal] ──────────────► [disputed] ──────────────► [normal]
                               │
                               │ chargeback
                               ▼
                          [chargebacked] (final — no re-disputes)
```

**Error handling** — Malformed CSV rows are logged to stderr and skipped. Invalid business operations (insufficient funds, nonexistent tx, wrong client) are silently ignored per the spec. I/O errors propagate via `Result`.

### Safety

- No `unsafe` code, no `unwrap()` on external input, no panics in the engine
- Zero/negative amounts rejected — prevents negative-deposit exploits
- Locked accounts reject all deposits and withdrawals
- Errors go to stderr; stdout is clean CSV only

### Efficiency

| Structure | Bounded By | Worst Case Memory |
|---|---|---|
| `accounts` | `u16` (65K clients) | ~5 MB |
| `transactions` | `u32` (4.3B deposits) | ~240 GB |

The transaction map is the bottleneck — we must store deposits for dispute lookups. A production system would use a disk-backed store. Withdrawals are not stored, which helps significantly in practice.

For server use: `PaymentsEngine::process()` is source-agnostic (takes a single record, not a reader). Could be sharded by `client_id` with zero cross-shard contention since disputes only reference same-client transactions.

## Assumptions

- **Locked accounts** reject new deposits and withdrawals (banking convention)
- **Client ID mismatch** on dispute/resolve/chargeback → ignored
- **Duplicate tx IDs** → ignored
- **Negative available balance** is possible after disputing a partially-withdrawn deposit — this is correct (the client is overdrawn)
- **Only deposits are disputable** — withdrawals are not stored in the transaction map

## Testing

**24 unit tests** (`engine.rs`) — every transaction type, dispute lifecycle, edge cases (wrong client, locked account, duplicate tx, precision, negative balance from disputes).

**6 integration tests** (`tests/integration.rs`) — end-to-end through the binary against CSV fixtures in `tests/fixtures/`.

```sh
cargo test
```

## AI Disclosure

Built with Claude Code (Anthropic). Used for architecture planning, code generation, and test design. All technical decisions were reviewed and are understood by me — I can discuss any aspect in detail.
