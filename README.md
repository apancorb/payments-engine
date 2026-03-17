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

## Correctness

Correctness is ensured through three complementary strategies:

### 1. Type System Guarantees

- **`rust_decimal::Decimal`** eliminates floating-point rounding errors entirely. Financial amounts are stored as exact decimal values, not IEEE 754 floats. For example, `0.1 + 0.2 == 0.3` holds true with `Decimal` but fails with `f64`.
- **`TransactionType` enum** with `#[serde(rename_all = "lowercase")]` means invalid transaction types are rejected at deserialization — not checked with string comparisons.
- **`Option<Decimal>` for amount** encodes in the type system that disputes/resolves/chargebacks don't carry amounts, while deposits/withdrawals do.
- **`u16` for client IDs and `u32` for tx IDs** match the spec exactly — overflow is impossible for valid inputs.

### 2. State Machine for Disputes

Each stored transaction tracks its dispute lifecycle via two booleans: `disputed` and `chargebacked`. The valid state transitions are:

```
                  dispute          resolve
  [normal] ──────────────► [disputed] ──────────────► [normal]
                               │
                               │ chargeback
                               ▼
                          [chargebacked] (final — no further disputes allowed)
```

The guards in each handler enforce these transitions:
- `dispute`: requires `!disputed && !chargebacked`
- `resolve`: requires `disputed`
- `chargeback`: requires `disputed`, then sets `chargebacked = true`

This prevents double-disputes, resolving non-disputed transactions, and re-disputing after a chargeback (which the spec calls "the final state").

### 3. Comprehensive Unit Tests (24 tests)

Every transaction type and edge case has a dedicated test that asserts exact `Decimal` values for `available`, `held`, and `total`. Tests are co-located in `engine.rs` with the logic they verify, making it easy to see what each function is supposed to do.

## Safety & Robustness

### No `unsafe` Code

The entire codebase uses safe Rust. No `unsafe` blocks, no raw pointers, no `unwrap()` on user-derived data. Rust's ownership system guarantees memory safety and prevents data races at compile time.

### No `unwrap()` / `expect()` on External Input

All external data flows through fallible APIs:
- **CSV parsing**: `csv_reader.deserialize()` returns `Result` — malformed rows are logged to stderr and skipped, never panicking. A single bad row doesn't halt processing of the remaining file.
- **File I/O**: `File::open()` and all `writeln!()` calls propagate errors via `?` to `run()`, which returns `Result<(), Box<dyn std::error::Error>>`.
- **Amount field**: `Option<Decimal>` — if a deposit/withdrawal is missing an amount, the `None` case is handled and the transaction is skipped.

### Invalid Amounts

Deposits and withdrawals with zero or negative amounts are rejected (line 44: `a > Decimal::ZERO`). This prevents a malicious actor from depositing negative amounts to drain an account.

### Locked Account Enforcement

Once a chargeback locks an account, both `deposit()` and `withdrawal()` check `account.locked` and return early. There's no code path that can bypass this — the check happens before any balance modification.

### No Panics in the Engine

The `PaymentsEngine::process()` method never panics. Every handler uses `match` with a catch-all `_ => return` for invalid states. The engine is designed to be resilient to malformed or adversarial input — it simply ignores what it can't process.

### Errors Go to stderr, Output Goes to stdout

Errors and warnings are written to stderr (`eprintln!`), while the CSV output goes to stdout. This means `cargo run -- input.csv > output.csv` captures only clean output — error messages don't corrupt the CSV.

## Assumptions

- **Locked accounts** reject all new deposits and withdrawals (banking convention — a frozen account cannot transact).
- **Client ID mismatch**: A dispute/resolve/chargeback referencing a transaction belonging to a different client is ignored.
- **Duplicate transaction IDs**: A deposit or withdrawal with an already-seen tx ID is ignored.
- **Negative available balance**: A dispute on a deposit that was partially withdrawn will make `available` go negative. This is correct — the funds are held and the total remains accurate.
- **Only deposits can be disputed**: The spec describes disputes referencing prior transactions by ID. Since we only store deposits in the transaction map, only deposits are disputable.

## Testing

### Unit Tests (24 tests in `engine.rs`)

Cover every transaction type and edge case:
- Deposit/withdrawal basics (single, multiple, exact balance, insufficient funds, zero amount, no prior deposit)
- Full dispute lifecycle (dispute → resolve, dispute → chargeback)
- Re-dispute after resolve (allowed), re-dispute after chargeback (rejected — final state)
- Double dispute on same tx (ignored)
- Multiple disputes on different txs for the same client
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
