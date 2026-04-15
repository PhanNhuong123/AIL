# Getting Started with AIL

AIL (AI Language) is a domain-specific language that lets you write programs as structured English
sentences. The AIL compiler verifies contracts with Z3, then emits production-ready Python and
auto-generated pytest stubs.

This guide walks you from installation through your first verified AIL function.

---

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | 1.75+ | [rustup.rs](https://rustup.rs) |
| Python | 3.8+ | [python.org](https://python.org) |
| pytest | any | `pip install pytest` |

Optional — for Z3 formal verification:

```bash
# Linux / macOS
sudo apt install z3   # or: brew install z3

# Windows — set LIBCLANG_PATH to your LLVM bin folder before building
```

---

## 1. Build and Install

```bash
git clone https://github.com/yourusername/ail
cd ail
cargo build --release
```

Add the binary to your PATH:

```bash
# Linux / macOS
export PATH="$PATH:$(pwd)/target/release"

# Windows (PowerShell)
$env:PATH += ";$(pwd)\target\release"
```

Verify:

```bash
ail --version
```

---

## 2. Create a New Project

```bash
ail init hello_wallet
cd hello_wallet
```

This creates:

```
hello_wallet/
├── ail.config.toml       ← project config
├── src/
│   └── main.ail          ← your AIL source files go here
├── generated/            ← AIL-owned output (overwritten on every build)
│   ├── __init__.py
│   ├── types.py
│   ├── functions.py
│   ├── test_contracts.py
│   └── functions.ailmap.json
└── scaffolded/           ← developer-owned (written once, then yours)
    └── __init__.py
```

`ail.config.toml` contains:

```toml
[project]
name = "hello_wallet"
version = "0.1.0"

[build]
target = "python"
contracts = "on"
source_map = true
async = false
```

---

## 3. Write Your First Types

Replace `src/main.ail` with these three files.

**`src/wallet_balance.ail`** — a semantic type with a constraint:

```
define WalletBalance:number where value >= 0
```

**`src/positive_amount.ail`** — a second constrained type:

```
define PositiveAmount:number where value > 0
```

**`src/user.ail`** — a record type:

```
describe User as
  balance:WalletBalance
```

AIL has no raw primitives. Every value has a named type with attached constraints. The compiler
propagates these constraints automatically — anywhere `WalletBalance` is used, `value >= 0` is
verified.

---

## 4. Write Your First Function

**`src/deduct_money.ail`**:

```
do deduct money
  from balance:WalletBalance, amount:PositiveAmount
  -> WalletBalance

  promise before: balance >= amount
  promise before: amount > 0
  promise after: balance >= 0

  let new_balance:WalletBalance = balance - amount
```

**Reading this:**

| Line | Meaning |
|------|---------|
| `do deduct money` | function name |
| `from balance:WalletBalance, amount:PositiveAmount` | typed parameters |
| `-> WalletBalance` | return type |
| `promise before: ...` | precondition — verified before execution |
| `promise after: ...` | postcondition — verified after execution |
| `let new_balance:... = ...` | computation with result type |

---

## 5. Build to Python

```bash
ail build
```

AIL runs the full pipeline:

```
Parse → Validate → Type-check → Z3 verify → Emit Python
```

Check `generated/functions.py`:

```python
def deduct_money(balance: WalletBalance, amount: PositiveAmount) -> WalletBalance:
    assert balance >= amount    # before: balance >= amount
    assert amount > 0           # before: amount > 0
    new_balance: WalletBalance = balance - amount
    assert new_balance >= 0     # after: new_balance >= 0
    return new_balance
```

Check `generated/types.py`:

```python
WalletBalance = NewType("WalletBalance", float)   # constraint: value >= 0
PositiveAmount = NewType("PositiveAmount", float)  # constraint: value > 0
```

---

## 6. Verify Contracts with Z3

Run the static verifier (no output emitted):

```bash
ail verify
```

If Z3 is installed, contracts are formally verified. Try introducing a violation — add
`promise before: balance < 0` to `deduct_money.ail` and run `ail verify` again. The compiler will
report the broken contract and a counterexample.

```bash
ail status    # show pipeline stage reached and graph statistics
```

---

## 7. Run Generated Tests

```bash
ail test
```

This builds, then runs `pytest` on `generated/test_contracts.py`. The generated stubs use
`pytest.skip()` — they document the contract shape but pass until you implement them:

```python
class TestDeductMoneyContracts:
    def test_pre_balance_ge_amount(self):
        pytest.skip("implement: balance >= amount")

    def test_post_balance_ge_0(self):
        pytest.skip("implement: balance >= 0")
```

Replace the `pytest.skip()` calls in the **scaffolded/** directory with real assertions. AIL
never overwrites `scaffolded/` after the first write.

---

## 8. Progressive Decomposition

AIL supports unlimited decomposition depth. The **filesystem structure IS the graph** — folders
are concept layers.

For a more complex function, split it across files using the naming convention
`NN_step_name.ail`:

```
src/
├── transfer_money.ail           ← top-level function declaration
└── transfer_money/              ← child steps (decomposition)
    ├── 01_validate.ail          ← check sender_id is not receiver_id
    ├── 02_fetch_sender.ail      ← fetch sender:User from database
    ├── 03_fetch_receiver.ail
    ├── 04_compute_sender_balance.ail   ← let new_sender_balance:WalletBalance = ...
    ├── 05_compute_receiver_balance.ail
    ├── 06_persist/              ← nested decomposition
    │   ├── 01_save_sender.ail
    │   └── 02_save_receiver.ail
    └── 07_return_result.ail
```

Step files use the same AIL patterns:

```
check sender_id is not receiver_id
  otherwise raise InvalidTransferError carries user_id = sender_id
```

```
let new_sender_balance:WalletBalance = sender.balance - amount
```

The CIC engine (Constraint Inheritance Chain) automatically propagates parent constraints
to every child step — you declare a constraint once at the top and the system guarantees it
holds at every depth.

---

## 9. MCP Integration (AI Tools)

`ail serve` starts an MCP server so Claude, Cursor, and other AI tools can query your AIL
project:

```bash
ail serve
```

Add to your AI tool's MCP config:

```json
{
  "mcpServers": {
    "ail": {
      "command": "ail",
      "args": ["serve"]
    }
  }
}
```

Available MCP tools:

| Tool | What it does |
|------|-------------|
| `ail.search` | BM25 full-text search over the project |
| `ail.context` | Return CIC context packet for a specific node |
| `ail.verify` | Run Z3 verification on a function |
| `ail.build` | Trigger a build and return generated file paths |
| `ail.status` | Return pipeline stage and graph statistics |

---

## CLI Reference

```bash
ail init <name>          # scaffold a new project
ail build                # full pipeline → generated Python
ail build --watch        # rebuild on file change
ail verify [file]        # run verification without emitting
ail test                 # build + run pytest
ail status               # show pipeline stage and graph stats
ail serve                # start MCP server over stdio
```

Build flags:

```bash
ail build --contracts on       # emit assert statements (default)
ail build --contracts comments # emit contracts as comments only
ail build --contracts off      # omit contracts from output
ail build --source-map         # generate functions.ailmap.json
```

---

## Next Steps

- Read [docs/AIL-Rules-v1.0.md](docs/AIL-Rules-v1.0.md) for the full syntax reference (17 patterns).
- Read [docs/AIL-Spec-v1.0.md](docs/AIL-Spec-v1.0.md) for the crate API, error catalog, and JSON format.
- Browse the complete `wallet_service` example in
  [crates/ail-graph/tests/fixtures/wallet_service/](crates/ail-graph/tests/fixtures/wallet_service/).
- Install the Python runtime helpers:
  ```bash
  pip install -e crates/ail-runtime-py
  ```
