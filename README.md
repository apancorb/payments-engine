# Payments Engine

A toy payments engine in Rust that reads transactions from a CSV file, processes deposits, withdrawals, disputes, resolves, and chargebacks, then outputs the final client account states.

## Usage

```sh
cargo run -- transactions.csv > accounts.csv
```

## Design

### Architecture

The codebase is split into three modules:

- **`types.rs`** — Data types: `TransactionType` enum, `InputRecord` (CSV deserialization), `Account`, `StoredTransaction`, and `OutputRecord` (CSV output with 4-decimal formatting).
- **`engine.rs`** — `PaymentsEngine` holds all state (`HashMap<u16, Account>` for clients, `HashMap<u32, StoredTransaction>` for dispute lookups) and processes each transaction type.
- **`main.rs`** — CLI entry point. Streams CSV records through the engine one at a time, then writes output to stdout.

### Precision

All monetary values use `rust_decimal::Decimal` to avoid floating-point rounding errors. Output is formatted to exactly 4 decimal places.

### Streaming

The CSV is read record-by-record using a `BufReader`, so memory usage stays constant regardless of input size. Only deposits are stored in the transaction map (for dispute resolution); withdrawals are not retained since they cannot be disputed.

### Error Handling

- Malformed CSV rows are logged to stderr and skipped — they don't halt processing.
- Invalid operations (insufficient funds, duplicate tx IDs, disputes on nonexistent transactions, client ID mismatches) are silently ignored per the specification.
- The `run()` function returns `Result` for clean propagation of I/O errors.

## Assumptions

- **Locked accounts** reject all new deposits and withdrawals (banking convention — a frozen account cannot transact).
- **Client ID mismatch**: A dispute/resolve/chargeback referencing a transaction belonging to a different client is ignored.
- **Duplicate transaction IDs**: A deposit or withdrawal with an already-seen tx ID is ignored.
- **Negative available balance**: A dispute on a deposit that was partially withdrawn will make `available` go negative. This is correct — the funds are held and the total remains accurate.
- **Only deposits can be disputed**: The spec describes disputes referencing prior transactions by ID. Since we only store deposits in the transaction map, only deposits are disputable.

## Testing

### Unit Tests (18 tests in `engine.rs`)

Cover every transaction type and edge case:
- Deposit/withdrawal basics (single, multiple, exact balance, insufficient funds)
- Full dispute lifecycle (dispute → resolve, dispute → chargeback)
- Edge cases: nonexistent tx, non-disputed tx, wrong client ID, locked accounts, duplicate tx IDs
- Precision with 4 decimal places
- Negative available balance from disputed partial withdrawal
- The exact example from the specification

### Integration Tests (6 tests in `tests/integration.rs`)

End-to-end tests that run the binary against CSV fixture files:
- PDF specification example
- Dispute → resolve lifecycle
- Chargeback → account freeze lifecycle
- Multiple interleaved clients
- Empty input file
- Whitespace-heavy CSV

Run all tests:

```sh
cargo test
```

## AI Disclosure

This project was built with the assistance of Claude (Anthropic's AI). Specifically, I used Claude Code (CLI tool) to help with:

- **Architecture planning** — Discussed the module structure, type design, and edge case handling before writing code.
- **Code generation** — Claude generated the initial implementation of all three modules (`types.rs`, `engine.rs`, `main.rs`), test fixtures, and integration tests.
- **Test design** — The unit and integration test cases were designed collaboratively, covering edge cases like client ID mismatches, negative balances from disputes, and locked account behavior.

All technical decisions (choice of `rust_decimal` over `f64`, streaming design, dispute state machine, assumptions about locked accounts) were reviewed and understood by me. I can discuss any aspect of the implementation in detail.
