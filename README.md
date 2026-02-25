# Accounts Manager

A toy payments engine that reads transactions from CSV, processes client account operations (deposits, withdrawals, disputes, resolves, chargebacks), and outputs final account states.

## Usage

```bash
cargo run -- input.csv > accounts.csv
```

**Input**: CSV with columns `type, client, tx, amount`
**Output**: CSV with columns `client, available, held, total, locked`

### Demo

Given `input.csv`:
```csv
type,       client, tx, amount
deposit,         1,  1,  50.00
withdrawal,      1,  2,  20.00
deposit,         2,  3,  99.00
deposit,         1,  4,  30.00
dispute,         1,  4,
resolve,         1,  4,
deposit,         2,  5,  75.50
dispute,         2,  5,
chargeback,      2,  5,
withdrawal,      1,  6,  98.00
```

Running `cargo run -- input.csv` produces:
```csv
client,available,held,total,locked
1,60,0,60,false
2,99.0,0.0,99.0,true
```

> **Note**: 12 integration tests in `tests/integration.rs` already serve as proof of correctness.

> Moreover, a demo input.csv file is supplied.

## Design Decisions

- **`rust_decimal`** for currency math, avoids floating-point precision errors inherent to `f64`
- **Streaming processing**, records are read and processed one-at-a-time via a `for` loop over the CSV reader's iterator; only deposit metadata is stored for dispute lookups. Scales to large files without loading everything into memory
- **Silent error handling for invalid operations**, per the spec, malformed disputes/resolves/chargebacks (wrong tx, wrong client, wrong state) are silently ignored. Malformed CSV rows are logged to stderr and skipped
- **Locked accounts**, after a chargeback, all further operations (deposits, withdrawals, disputes, resolves, chargebacks) on the frozen account are ignored
- **`thiserror`** for error type derivation, replaces boilerplate `impl Display/Error/From` with a clean derive macro
- **`clap`** (derive) for CLI parsing, provides `--help`, argument validation, and clear error messages

## Transaction Types

| Type | Effect |
|------|--------|
| `deposit` | Credits available and total funds |
| `withdrawal` | Debits available and total (fails silently if insufficient funds) |
| `dispute` | Moves disputed amount from available to held |
| `resolve` | Moves disputed amount from held to available |
| `chargeback` | Removes held amount from total, locks account |

## Assumptions

- Only **deposit** transactions can be disputed (withdrawals are not stored for dispute lookup)
- A transaction can only be disputed once at a time (duplicate disputes are ignored)
- Disputes must come from the **same client** that owns the transaction
- Locked accounts reject all further operations (deposits, withdrawals, disputes, resolves, chargebacks)
- Malformed CSV rows are skipped with a stderr warning

## Correctness Guarantees

The engine enforces several critical invariants to prevent fraud and data corruption:

- **No double-disputes**
  `if stored.under_dispute { return; }`
  Without this, a malicious actor could dispute the same transaction repeatedly, draining `available` into `held` beyond the original amount.

- **Resolve/chargeback require active dispute**
  `if !stored.under_dispute { return; }`
  Prevents releasing or charging back funds that were never held.

- **Client ownership validation**
  `if stored.client != record.client { return; }`
  Prevents cross-client fraud (e.g. client 2 disputing client 1's deposit).

- **Locked account protection**
  `if account.locked { return; }`
  A frozen account (post-chargeback) accepts no further operations of any kind.

## Project Structure

```
src/
├── main.rs     # CLI entry point (clap)
├── types.rs    # Domain types (TransactionRecord, ClientAccount, etc.)
├── engine.rs   # Core processing logic
└── error.rs    # Custom error type (thiserror)
tests/
└── integration.rs  # 12 test cases
```

## Testing

```bash
cargo test
```

Tests cover: basic deposits/withdrawals, insufficient funds, dispute -> resolve, dispute -> chargeback, nonexistent tx disputes, unauthorized disputes, locked account behavior, decimal precision, and whitespace tolerance.

## AI Usage

AI assisted throughout development in the following ways:

**Edge case identification**, helped identify correctness-critical cases that go beyond the basic happy path:
- Disputes referencing a transaction owned by a different client
- Chargebacks/resolves on transactions that are not under active dispute
- Deposits and disputes on an already-frozen (post-chargeback) account
- Repeated disputes on the same transaction

All of these are verified by dedicated tests in `tests/integration.rs`.

**Test scaffolding**, generated the initial structure of integration tests, which were then reviewed, corrected for `rust_decimal` trailing-zero normalization behavior, and expanded.

**README.md formatting**, formatted the README.md file to be more readable and organized.

## Development Tools

VS Code extensions used during development:

| Extension | Purpose |
|-----------|---------|
| [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) | Rust language server, inline errors, completions, go-to-definition |
| [Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml) | `Cargo.toml` syntax highlighting and validation |
| [crates](https://marketplace.visualstudio.com/items?itemName=serayuzgur.crates) | Shows latest available crate versions inline in `Cargo.toml` |
| [Rainbow CSV](https://marketplace.visualstudio.com/items?itemName=mechatroner.rainbow-csv) | Colour-codes CSV columns for readable inspection of input/output data |
| [Error Lens](https://marketplace.visualstudio.com/items?itemName=usernamehw.errorlens) | Surfaces compiler errors and warnings inline at the offending line |
| [CodeLLDB](https://marketplace.visualstudio.com/items?itemName=vadimcn.vscode-lldb) | Native Rust debugger, breakpoints, variable inspection, step-through |
| [Dependi](https://marketplace.visualstudio.com/items?itemName=fill-labs.dependi) | Dependency management, vulnerability alerts, outdated crate detection |