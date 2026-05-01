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

`z3 0.20+` (used by `ail-contract --features z3-verify`) bundles Z3 4.16 source
via the `z3-src` crate and ships static FFI bindings, so `libclang`/`LLVM` is
**no longer required**. A working C++ toolchain (MSVC on Windows, gcc/clang on
Linux/macOS) plus CMake is all you need:

```bash
# Build with formal verification enabled
cargo build --release --features z3-verify
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

Note: only `[database] backend` is read by the CLI today — `[build]` fields are placeholders for future config-driven mode. Use CLI flags (`--target`, `--contracts`, `--source-map`) to control build behavior. See `docs/config-reference.md` for the full status table.

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

> **Scope tip (closes review finding F1).** `ail verify [path]` in v0.1 always
> verifies the **whole project**; the `path` argument selects the project
> root, not a node-level filter. Run it from inside the project directory
> (the one containing `ail.config.toml`), not from a parent workspace that
> contains multiple projects — otherwise the CLI will scan every `.ail`
> file it can reach (including any test fixtures in sibling crates) and may
> surface unrelated errors. The IDE always invokes the verifier with an
> explicit project root, so this only matters for terminal use.

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

v2.0 adds 5 write tools — see §14 MCP Write Tools.

---

## 10. SQLite Migration (v2.0)

By default AIL reads `.ail` text files. v2.0 adds an opt-in SQLite backend
that stores the graph, contracts, FTS5 index, and CIC cache in a single
`project.ail.db` file.

```bash
cd hello_wallet
ail migrate --from src/ --to project.ail.db --verify
```

`--verify` re-parses the source after migration and confirms node, edge, and
contract counts match. Add the SQLite working files to `.gitignore`:

```
project.ail.db-wal
project.ail.db-shm
```

Make the SQLite preference explicit (optional):

```toml
[database]
backend = "auto"   # auto | sqlite | filesystem
```

`auto` selects SQLite when `project.ail.db` exists next to `ail.config.toml`.
The rest of the pipeline runs unchanged: `ail verify`, `ail build`,
`ail test`.

To roll back, delete `project.ail.db` (and the `-wal` / `-shm` siblings).
The filesystem `.ail` tree is untouched by `ail migrate`.

To inspect what is in the database:

```bash
ail export --from project.ail.db --to exported/
```

Output is a single `exported/export.ail` file, not a per-node tree.

---

## 11. TypeScript Build (v2.0)

```bash
ail build --target typescript
```

This writes `dist-ts/`:

```
dist-ts/
├── tsconfig.json        # strict
├── package.json
├── ail-runtime.ts       # inlined runtime
├── types/               # one file per Define / Describe
├── errors/              # one file per Error node
├── fn/                  # one file per top-level Do
└── tests/               # Vitest stubs
```

Run it with Node:

```bash
cd dist-ts
npm install
npx tsc --noEmit
npx vitest run
```

Define types are emitted as branded factories that enforce the parsed
`where` constraint at runtime:

```ts
import { createWalletBalance } from "./types/wallet_balance";

createWalletBalance(10);   // ok
createWalletBalance(-1);   // throws — value >= 0 violated
```

---

## 12. Search (v2.0)

`ail search` runs over the SQLite backend. BM25 is always available; hybrid
search requires the `embeddings` Cargo feature plus an ONNX model.

BM25 (no extra setup beyond the SQLite backend):

```bash
ail search "balance transfer"
ail search "transfer" --budget 5
```

Hybrid (BM25 + ONNX semantic, fused with RRF):

```bash
cargo build -p ail-cli --features embeddings --release
ail search --setup
# place model files at ~/.ail/models/all-MiniLM-L6-v2/
ail reindex --embeddings
ail search "balance transfer" --semantic
```

`ail search --bm25-only` forces BM25 even when `--semantic` is also passed.

---

## 13. CIC Context Packets (v2.0)

```bash
ail context --task "validate transfer"          # BM25-pick the top Do node
ail context --node transfer_money               # target a specific node
```

Requires the SQLite backend (filesystem-only projects return an error with a
remediation hint). The response includes:

- the target node and its inherited constraints (CIC down/up/across/diagonal),
- `promoted_facts`: facts proven on the prevailing path that the engine has
  promoted forward (path-sensitive CIC),
- a `cache_hit` indicator. The first call seeds the `cic_cache` table; the
  second call for the same node hits the cache.

---

## 14. MCP Write Tools (v2.0)

`ail serve` now exposes 10 MCP tools — 5 read tools plus 5 write tools.

| Tool          | Purpose                                                |
|---------------|--------------------------------------------------------|
| `ail.write`   | Create a new node (accepts `metadata`).                |
| `ail.patch`   | Modify an existing node (`path` / `value`).            |
| `ail.move`    | Re-parent or reorder a node.                           |
| `ail.delete`  | Remove a node (`cascade`, `orphan`, `dry_run`).        |
| `ail.batch`   | Ordered atomic multi-op; rollback on first error.      |

`ail.batch` snapshots the in-memory `AilGraph` (via `Clone`) before running
ops and restores the snapshot if any op fails. `dry_run` is rejected inside
a batch.

`ail.verify` and `ail.build` are dirty-aware: after any write tool, the
next verify/build re-pipelines from the in-memory graph rather than
re-parsing from disk. Edits live in-memory within a session;
`SqliteGraph::save_from_graph()` is available for explicit disk persistence.

---

## CLI Reference

**Core pipeline**

```bash
ail init <name>                          # scaffold a new project
ail build [--target python|typescript]   # full pipeline → generated source
ail build --watch                        # rebuild on file change
ail build --contracts on|comments|off    # contract emission mode
ail build --source-map                   # write functions.ailmap.json
ail build --from-db <path>               # build from a specific .ail.db
ail verify [file] [--from-db <path>]     # run pipeline without emitting
ail test                                 # build + run pytest
ail status                               # pipeline stage + graph stats
```

**Migration**

```bash
ail migrate --from <src/> --to <db> [--verify]   # filesystem → SQLite
ail export --from <db> --to <dir>                 # SQLite → export.ail
```

**Search**

```bash
ail search <query>                       # BM25 (requires SQLite)
ail search <query> --semantic            # hybrid RRF (requires embeddings feature)
ail search <query> --bm25-only           # force BM25 even if --semantic given
ail search --setup                       # check ONNX model files
ail search <query> --budget <n>          # cap result count
ail reindex                              # clear embedding vectors
ail reindex --embeddings                 # rebuild embedding index
```

**CIC context**

```bash
ail context --task "<text>" [--from-db <path>]   # BM25-pick a Do node, print packet
ail context --node <name>   [--from-db <path>]   # target a node by name
```

**MCP server**

```bash
ail serve                                # start MCP server over stdio
```

---

## Next Steps

- Read [MIGRATION.md](MIGRATION.md) for the v1.0 → v2.0 upgrade guide.
- Read [docs/config-reference.md](docs/config-reference.md) for the full
  `ail.config.toml` field status table.
- Browse the canonical end-to-end example in
  [examples/wallet_service/](examples/wallet_service/).
- Read [docs/plan/v1.0/reference/AIL-Rules-v1.0.md](docs/plan/v1.0/reference/AIL-Rules-v1.0.md) for the full syntax reference (17 patterns).
- Read [docs/plan/v1.0/reference/AIL-Spec-v1.0.md](docs/plan/v1.0/reference/AIL-Spec-v1.0.md) for the crate API, error catalog, and JSON format.
- Install the Python runtime helpers:
  ```bash
  pip install -e crates/ail-runtime-py
  ```
