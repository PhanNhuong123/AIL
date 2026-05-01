# CLAUDE.md

Guidance for Claude Code (and other AI coding assistants) working in this repository.

> **Note:** This file is listed in `.gitignore` as a private/internal agent doc.
> Edit it locally; it is not meant to be committed.

---

## 1. What this repo is

**AIL — AI Layer.** A correctness layer that sits between an AI coding
assistant and application code. Users describe a system in `.ail` files
(structured-English DSL); AIL parses them into a graph, propagates constraints
through CIC, type-checks, runs Z3 verification, then emits production code
(Python today, TypeScript today, Rust later) with auto-generated tests and a
source map back to the originating `.ail` node.

The hard pipeline is enforced by Rust types and is one-way:

```
.ail text / .ail.db
  → AilGraph → ValidGraph → TypedGraph → VerifiedGraph
  → emit (Python | TypeScript) | MCP response | IDE JSON
```

You **cannot** emit unverified code — the type signatures forbid it. Any
new pipeline entrypoint must respect this gate ordering.

Current state: **v3.0.0** (semantic coverage + agent foundation). The
desktop IDE (Tauri + Svelte) is the v4.0 target; treat it as in-progress.

---

## 2. Repo layout

```
AIL/
├── Cargo.toml                # Rust workspace (11 crates) — Tauri-free
├── crates/
│   ├── ail-graph/            PSSD graph core, 17 patterns, 3 edge kinds, CIC, BM25
│   ├── ail-types/            Type system, constraint/value AST, type checker
│   ├── ail-contract/         Static checks + Z3 encode/verify + Čech sheaf
│   ├── ail-text/             pest parser (.ail) + renderer
│   ├── ail-emit/             Python + TypeScript emitters, scaffold, source map
│   ├── ail-db/               SQLite GraphBackend, FTS5, CIC/coverage/embedding caches
│   ├── ail-search/           BM25 + ONNX embeddings, hybrid RRF
│   ├── ail-coverage/         Semantic coverage (parent intent vs children)
│   ├── ail-mcp/              MCP JSON-RPC server (read + write tools)
│   ├── ail-cli/              `ail` binary — init/build/verify/test/migrate/...
│   ├── ail-ui-bridge/        JSON bridge + optional Tauri commands for the IDE
│   └── ail-runtime-py/       Python runtime helper (MIT, separate license)
├── agents/                   Python LangGraph agent (ail_agent / `ail-agent` binary)
├── ide/                      Tauri v2 + SvelteKit desktop IDE (NOT in workspace)
│   └── src-tauri/            Standalone Cargo project — must stay out of root workspace
├── examples/wallet_service/  Canonical end-to-end example used by E2E tests
├── wiki/                     Vietnamese architecture wiki (current source of truth)
├── .github/workflows/ci.yml  CI: build + clippy + fmt + tests + frozen-binary
├── README.md / GETTING_STARTED.md / MIGRATION.md / CHANGELOG.md / DESIGN.md
└── CLAUDE.md (this file)
```

**Internal crate dependency map** (from `wiki/02-rust-core.md`):

| Crate            | Depends on                                  |
|------------------|---------------------------------------------|
| `ail-graph`      | (none — base)                               |
| `ail-db`         | `ail-graph`                                 |
| `ail-types`      | `ail-graph`                                 |
| `ail-contract`   | `ail-graph`, `ail-types`                    |
| `ail-text`       | `ail-graph`, `ail-types`                    |
| `ail-emit`       | `ail-graph`, `ail-types`, `ail-contract`    |
| `ail-search`     | `ail-graph` (+ optional ONNX)               |
| `ail-coverage`   | `ail-graph`, `ail-search`                   |
| `ail-mcp`        | most cores; optional `ail-db`/`ail-coverage`|
| `ail-cli`        | most cores                                  |
| `ail-ui-bridge`  | `ail-graph`, `ail-types`, `ail-contract`, `ail-text` |

---

## 3. The 17 patterns (closed vocabulary)

All `.ail` files are built from exactly these 17 `Pattern` variants
(`crates/ail-graph/src/types/pattern.rs`):

```
Define, Describe, Error,                  // type-defining
Do, Promise,                              // structural
Let, Check, ForEach, Match,               // computation / control
Fetch, Save, Update, Remove,              // persistence
Return, Raise, Together, Retry            // termination / orchestration
```

Edges (`crates/ail-graph/src/types/edge.rs`):

| Edge | Direction        | Meaning                                              |
|------|------------------|------------------------------------------------------|
| `Ev` | parent → child   | structural decomposition (filesystem hierarchy = graph) |
| `Eh` | sibling → sibling| ordering inside a parent (file order, step order)    |
| `Ed` | cross-reference  | type/error/template/call/shared-pattern references   |

When you add a new node kind or syntax, you almost certainly want to
extend an existing pattern instead of inventing a new one. New patterns
require coordinated changes in `ail-graph`, `ail-text/grammar.pest`,
`ail-types`, `ail-contract`, and every emitter.

---

## 4. CIC — Constraint Inheritance Chain

Implemented in `crates/ail-graph/src/cic/mod.rs`
(`compute_context_packet_for_backend`). Four propagation rules:

- **DOWN** — ancestor contracts → descendant `inherited_constraints`.
- **UP** — verified child postconditions become parent facts.
- **ACROSS** — sibling output / promoted facts flow to following siblings
  (path-sensitive; promoted by `Check ... otherwise raise ...`).
- **DIAGONAL** — type constraints + outgoing `Ed` (calls / templates /
  shared patterns) inject everywhere the type/symbol is used.

Cache lives in SQLite (`cic_cache` table, `crates/ail-db/src/db/cic_cache.rs`).
Invalidation must hit descendants, ancestors, next siblings + their
descendants, incoming `Ed` neighbors; `Check` nodes additionally
invalidate later siblings because they can promote facts forward.

If you change CIC, also re-check the cache invalidation paths
(`SqliteGraph::update_node`, MCP write tools) and `ail-db`
`invalidate_coverage_for_ancestors`.

---

## 5. Pipeline gates and how to add to them

Type-level stage gates — do not bypass:

| Stage    | Crate                                | Constructor                  |
|----------|--------------------------------------|------------------------------|
| Parse    | `ail-text` / `ail-db`                | `parse`, `parse_directory`, `SqliteGraph::open`/`load_into_graph` |
| Validate | `ail-graph::validation`              | `validate_graph -> ValidGraph` |
| Type     | `ail-types::checker`                 | `type_check -> TypedGraph`     |
| Verify   | `ail-contract::verify`               | `verify -> VerifiedGraph`      |
| Emit     | `ail-emit`                           | accepts `&VerifiedGraph` only  |

Existing pipeline drivers (use these as references when adding new ones):

- CLI: `crates/ail-cli/src/commands/{build.rs,verify.rs}`
- MCP refresh: `crates/ail-mcp/src/pipeline.rs`
- IDE bridge: `crates/ail-ui-bridge/src/pipeline.rs`

Z3 verification is gated by feature `ail-contract/z3-verify`. Without it,
only the static checks run; `cargo build`/`cargo test` defaults still
work without LLVM/libclang.

---

## 6. Build, test, lint

### Rust (workspace)

```bash
cargo build                                         # default features
cargo build --release
cargo build -p ail-contract --features z3-verify    # adds Z3 (needs cmake; no libclang since z3 0.20+)
cargo test --workspace                              # default features only
cargo test -p ail-cli                               # one crate
cargo test -p ail-cli --test cli_e2e_wallet_sqlite  # one test binary
cargo clippy -- -D warnings                         # CI gate
cargo fmt --all -- --check                          # CI gate
```

`ide/src-tauri/` is a **standalone Cargo project**, deliberately not a
workspace member. `cargo build --workspace` from the repo root must stay
Tauri-free; never add it to root `Cargo.toml`.

### Feature flags worth knowing

- `ail-contract/z3-verify` — Z3 encoding/verification + sheaf obstruction
  detection. CI builds this on Linux + Windows but skips macOS.
- `ail-search/embeddings`, `ail-cli/embeddings`, `ail-mcp/embeddings` —
  ONNX semantic search; needs the `all-MiniLM-L6-v2` model at
  `~/.ail/models/all-MiniLM-L6-v2/`. Many coverage tests are `#[ignore]`
  unless this feature + model are in place.
- `ail-ui-bridge/tauri-commands` — Tauri commands, watcher, async
  verifier, agent subprocess. Off by default.

### Python agent

```bash
pip install -e './agents/[dev]'              # editable + pytest
pip install -e './agents/[all,dev]'          # + every provider SDK
python -m pytest agents/tests/               # default suite
python -m pytest agents/tests/ -m integration   # live, needs API key + ail on PATH
```

Python tests follow this layout (see `agents/tests/`):

- `test_orchestrator.py`, `test_workflow_e2e_*.py` — LangGraph state machine
- `test_planner.py`, `test_plan_format.py`, `test_coder.py`, `test_verify.py` — workflow nodes
- `test_providers.py`, `test_provider_swap.py`, `test_registry.py`, `test_retry.py` — provider adapters
- `test_mcp_toolkit.py`, `test_cli_main.py`, `test_progress_json.py`, `test_errors.py` — surface
- `test_integration_live.py` — gated by `AIL_RUN_LIVE_INTEGRATION=1`
- `test_frozen_binary*.py` — gated by `AIL_FROZEN_BIN` env var (CI freezes via PyInstaller)

The CI workflow (`.github/workflows/ci.yml`) freezes a `ail-agent`
sidecar with PyInstaller and asserts a 350 MB hard cap.

### Generated project tests

```bash
cd examples/wallet_service
ail build                     # → generated/ + scaffolded/ (Python by default)
ail test                      # build + pytest generated/test_contracts.py
ail build --target typescript # → dist-ts/ ; then npm install && npx tsc --noEmit && npx vitest run
```

`AIL_SKIP_TS_NODE=1` skips Node-toolchain shell-outs in TS E2E tests.

### IDE (Tauri + SvelteKit)

```bash
cd ide
pnpm install
pnpm run dev          # SvelteKit only, port 1420 strict
pnpm tauri dev        # full desktop window (needs Rust toolchain)
pnpm run build        # static frontend → ide/build/
pnpm run check        # svelte-check
pnpm run test         # vitest
```

Windows desktop builds need WebView2; `cargo check` works without it.

---

## 7. CLI surface (the `ail` binary)

Operate from inside a project directory containing `ail.config.toml`. There
is no global `--project` flag — `cd` into the project first.

| Command                                  | Purpose                                                     |
|------------------------------------------|-------------------------------------------------------------|
| `ail init <name>`                        | Scaffold project (`src/main.ail`, `generated/`, `scaffolded/`, config). |
| `ail verify [path] [--from-db PATH]`     | Run pipeline; **always whole-project** (path is project root hint). |
| `ail build [--target python\|typescript]`| Pipeline → emit. Flags: `--watch`, `--contracts`, `--source-map`, `--from-db`. |
| `ail test`                               | Build + pytest on `generated/test_contracts.py`.            |
| `ail status`                             | Print highest stage reached + node/edge/do counts.          |
| `ail migrate --from <src/> --to <db>`    | Filesystem `.ail` → SQLite. Add `--verify` to round-trip check. |
| `ail export --from <db> --to <dir>`      | SQLite → single `<dir>/export.ail` file (not a tree).       |
| `ail search <q>`                         | BM25 (SQLite). `--semantic` adds ONNX hybrid (RRF).         |
| `ail reindex [--embeddings]`             | Clear / rebuild embedding vectors.                          |
| `ail context --node N` / `--task "..."`  | CIC context packet (SQLite-only).                           |
| `ail coverage --node N` / `--all` / `--warm-cache` | Semantic coverage. SQLite + embeddings to score.    |
| `ail sheaf [--node N] [--format json]`   | Čech nerve / obstruction (full diagnostics need `z3-verify`). |
| `ail serve`                              | MCP server over stdio (10 tools).                           |
| `ail agent "<task>"`                     | Spawn `python -m ail_agent` LangGraph workflow.             |

Backend resolution (`crates/ail-cli/src/commands/project.rs`):

1. `--from-db <path>` wins.
2. `[database] backend = "sqlite"` requires `project.ail.db` next to config.
3. `[database] backend = "filesystem"` forces `src/` (or root).
4. `auto` / missing → SQLite if `project.ail.db` exists, else filesystem.

`--check-breaking` and `--check-migration` are parsed but currently
return `NotImplemented`.

---

## 8. MCP server tools (10 total)

`ail serve` exposes JSON-RPC 2.0 over stdio. Adding/removing a tool means
updating `crates/ail-mcp/src/{server.rs,tools/}` *and* the agent toolkit
in `agents/ail_agent/mcp_toolkit.py`.

| Tool          | Group | Notes                                                     |
|---------------|-------|-----------------------------------------------------------|
| `ail.search`  | read  | BM25/hybrid; returns id/score/intent/pattern/path.        |
| `ail.review`  | read  | Coverage + missing-aspect for a target node (no Z3).      |
| `ail.context` | read  | CIC packet (primary, secondary, constraints, promoted).   |
| `ail.status`  | read  | Stage + node/edge/do counts + root id.                    |
| `ail.verify`  | pipe  | Re-runs verify; uses in-memory dirty graph if mutated.    |
| `ail.build`   | pipe  | Returns emitted-file metadata (does NOT write to disk).   |
| `ail.write`   | write | Create node under parent (pattern, intent, metadata).     |
| `ail.patch`   | write | Modify intent/expression/pattern/contracts/metadata.      |
| `ail.move`    | write | Re-parent or reorder.                                     |
| `ail.delete`  | write | `cascade` / `orphan` / `dry_run`.                         |
| `ail.batch`   | write | Ordered atomic ops, snapshot/rollback on first error.     |

Write tools demote `ProjectContext` to `Raw`, set the dirty flag, and
clear search/embedding caches. The next `ail.verify`/`ail.build` re-runs
the pipeline from the in-memory graph (no disk reload). Edits live in
the MCP session; `SqliteGraph::save_from_graph()` is the explicit
persistence path — `ail serve` does not write back automatically.

`ail.batch` rejects `dry_run` and rolls back via `AilGraph::clone()`
snapshot.

---

## 9. Python agent (`agents/`)

Architecture: thin orchestration shell over the Rust core.

```
ail agent "<task>"
  ─→ ail-cli subprocess-spawns python -m ail_agent
       ─→ get_provider("provider:model")
       ─→ MCPToolkit spawns `ail serve` over stdio
       ─→ LangGraph: plan → code → verify → done | error
            ↳ planner: provider.complete → strict JSON {steps:[...]}
            ↳ coder:   resolves parent_id, dispatches ail.write, budget guard
            ↳ verify:  ail.status sanity check + VERIFY_OK marker
```

Key files:

- `agents/ail_agent/__main__.py` — argparse entrypoint, exit codes.
- `agents/ail_agent/orchestrator.py` — JSON-safe `AILAgentState`, deterministic router, iteration guard (AIL-G0142).
- `agents/ail_agent/{planner,coder,verify,plan_format}.py` — workflow nodes.
- `agents/ail_agent/registry.py` — lazy provider registry.
- `agents/ail_agent/providers/` — anthropic / openai / deepseek / alibaba / ollama. SDKs are optional extras.
- `agents/ail_agent/mcp_toolkit.py` — sync facade over async `mcp==1.9.2`.
- `agents/ail_agent/progress.py` — `Progress` (text) and `JsonProgress` (Tauri sidecar).

Strict invariants:

- Workflow context is **per-process**: tests must call
  `clear_workflow_context()` to avoid leakage.
- The plan parser **rejects** `"keep"` as a contract kind — Rust
  `ContractKind` does not have it. Allowed contract kinds: `before`,
  `after`, `always`.
- Allowed plan patterns: `always`, `check`, `define`, `describe`, `do`,
  `explain`, `fix`, `let`, `raise`, `set`, `test`, `use`. (Mapped to
  Rust patterns by the coder.)
- Errors carry stable codes: `AIL-G0140` (provider config), `G0141`
  (routing), `G0142` (iteration cap), `G0143` (step budget), `G0144`
  (plan parse), `G0145` (MCP connection).

Exit codes (also in `agents/README.md`): `0` done, `1` agent error,
`2` bad invocation/model spec, `3` `MCPConnectionError`, `130` SIGINT.

`--mcp-port` is reserved for a future network MCP transport — current
implementation always uses stdio.

---

## 10. `ail.config.toml`

Per `docs/config-reference.md` (and `wiki/05-testing-config-operations.md`),
**only three sections are read at runtime today**:

| Section            | Status   | Consumer                                  |
|--------------------|----------|-------------------------------------------|
| `[database] backend` | ACTIVE | All backend-resolving commands.           |
| `[coverage]`       | ACTIVE   | `ail coverage` (`enabled`, `threshold_full`, `threshold_partial`, `extra_concepts`). |
| `[agent]`          | ACTIVE   | `ail agent` (`model`, `max_iterations`, `steps_per_plan`). |

`[project]`, `[build]`, `[build.typescript]`, `[search]` are
**documentation/schema placeholders**. Build target, contract emission
mode, source-map output are controlled by CLI flags
(`--target`, `--contracts`, `--source-map`). Unknown keys are silently
ignored (forward-compat). `timeout_seconds` under `[agent]` is reserved.

Agent config precedence: CLI flag → `[agent]` TOML → Python default.

---

## 11. The IDE (`ide/`)

Tauri v2 + SvelteKit, frontend in `ide/src/`, Rust shell in
`ide/src-tauri/` (out-of-workspace Cargo project). Glue is
`crates/ail-ui-bridge` behind feature `tauri-commands`.

Canonical v4 shell:

```
TitleBar
├─ Outline        # project tree, types, errors, filter
├─ Stage          # System / Module / Flow / Node views, lens-aware
└─ RightSidebar   # always-visible chat tab + collapsible rail
```

Stage view dispatch by `selection.kind`:
`project|none → SystemView`, `module → ModuleView`,
`function → FlowView`, `step → NodeView`.

Stable JSON contract is mirrored in `crates/ail-ui-bridge/src/serialize/`
and `ide/src/lib/types.ts` — **change both sides together** when adding
fields. The watcher (`ail-ui-bridge::watcher`) debounces `.ail` changes
and emits `graph-updated` patches; the route-level handler in
`ide/src/routes/+page.svelte` is the **only** place allowed to apply
patches to `graph` / `selection` stores. Chat preview cards dispatch
events upward, never mutate stores directly.

DESIGN.md describes the Apple-inspired visual language used by the IDE.

---

## 12. Conventions and "do not" list

- **Do not** add to or remove from the 17 patterns without a coordinated
  change across `ail-graph`, `ail-text` grammar, `ail-types`,
  `ail-contract`, both emitters, MCP write tools, and the Python plan
  parser.
- **Do not** add `ide/src-tauri` to the root Cargo workspace.
- **Do not** introduce new pipeline entrypoints that bypass
  `validate → type_check → verify`. Use the existing drivers as templates.
- **Do not** mutate the `graph` / `selection` Svelte stores from chat or
  child components — only the route-level handler may apply patches.
- **Do not** make MCP write tools implicitly persist to `.ail.db` —
  persistence is explicit (`SqliteGraph::save_from_graph`).
- **Do not** lose error codes (`AIL-G0140..G0145`) when refactoring agent
  errors — they are part of the public contract.
- **Do not** assume every test runs by default — many coverage / hybrid
  search / frozen-binary tests are `#[ignore]`d or `-m integration` and
  need feature flags or env vars.
- **Do** keep `cargo test --workspace` green without features; gate
  feature-specific tests on the feature.
- **Do** treat `examples/wallet_service` as the canonical fixture. CLI
  E2E tests copy it into a tempdir to keep the source clean — preserve
  that pattern.
- **Do** look at `wiki/01..06-*.md` first for architecture and conventions
  (Vietnamese). It is the most up-to-date narrative documentation in the
  repo and is what the wiki points at as canonical.
- **Do** prefer extending `EmitConfig`, `[agent]`, or existing TOML
  sections over inventing new top-level config.

---

## 13. Where to look first

Quick reference when answering "where is X":

| Question                                | First file to read                                         |
|-----------------------------------------|------------------------------------------------------------|
| What is the .ail surface syntax?        | `crates/ail-text/src/grammar.pest` + `GETTING_STARTED.md`  |
| How is a parsed `.ail` turned into a graph? | `crates/ail-text/src/parser/{walker,assembler,directory}.rs` |
| Where do constraints propagate?         | `crates/ail-graph/src/cic/mod.rs`                          |
| Where does Z3 actually run?             | `crates/ail-contract/src/z3_verify/mod.rs`, `z3_encode/`   |
| How is Python emitted?                  | `crates/ail-emit/src/python/`                              |
| How is TypeScript emitted?              | `crates/ail-emit/src/typescript/`                          |
| Where do MCP tools live?                | `crates/ail-mcp/src/{server.rs, tools/}`                   |
| Where is backend resolution?            | `crates/ail-cli/src/commands/project.rs`                   |
| Where is coverage scoring?              | `crates/ail-coverage/src/coverage.rs`                      |
| Where is the SQLite schema?             | `crates/ail-db/src/db/schema.rs`                           |
| Where is the agent state machine?       | `agents/ail_agent/orchestrator.py`                         |
| What does the IDE actually call into?   | `crates/ail-ui-bridge/src/{commands,pipeline,serialize}.rs`|
| Architecture overview (Vietnamese)      | `wiki/02-rust-core.md` (most detailed)                     |
| End-to-end product story                | `README.md`, then `GETTING_STARTED.md`                     |
| What changed in v3.0                    | `CHANGELOG.md` (`[3.0.0] - 2026-04-20`)                    |
| v1→v2 migration semantics               | `MIGRATION.md`                                             |

---

## 14. Licensing reminder

- Core: **BUSL-1.1**, with conversion to MIT on **2030-04-19** or four
  years after each version's first public release, whichever is sooner.
- `crates/ail-runtime-py` and generated runtime code: **MIT** immediately.
- Code generated from user-authored `.ail` input by unmodified AIL tools
  is not part of the Licensed Work.

When adding files, copy the appropriate `LICENSE` headers from a
neighbour. Do not mix BUSL files into `ail-runtime-py`.
