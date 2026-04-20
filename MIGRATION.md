# Migration Guide — AIL v1.0 to v2.0

AIL v2.0 adds a SQLite backend, a TypeScript emitter, embedding-based hybrid
search, MCP write tools for AI agents, and path-sensitive CIC context packets.
**No v1.0 workflows are removed.** Filesystem `.ail` projects continue to
build and verify exactly as they did in v1.0; every v2.0 feature below is
opt-in.

## What Changed in v2.0 (User-Visible)

| Area              | v1.0                          | v2.0                                                                  |
|-------------------|-------------------------------|-----------------------------------------------------------------------|
| Graph storage     | Filesystem `.ail` files only  | Filesystem (default) or SQLite via `ail migrate`                      |
| CLI commands      | 7 commands                    | 12 commands (adds `migrate`, `export`, `context`, `search`, `reindex`) |
| Build targets     | Python                        | Python (default) or TypeScript via `--target typescript`              |
| MCP tools         | 5 read tools                  | 10 tools (adds `write`, `patch`, `move`, `delete`, `batch`)           |
| Search            | None (use grep)               | BM25 over SQLite, plus optional ONNX-backed hybrid RRF                |
| CIC context       | Path-insensitive              | Path-sensitive with `promoted_facts` (additive)                       |

## No-Change Path (v1.0 Users Who Keep Filesystem)

Existing v1.0 commands continue to work without changes. None of the
following requires migration:

```bash
ail init <project>
ail build
ail verify
ail test
ail serve
ail status
```

If your project already has an `ail.config.toml` from v1.0, no edit is
required. The CLI defaults to the filesystem backend when `project.ail.db` is
absent.

## Opt-In Path 1 — Migrate to SQLite Backend

```bash
cd your_project
ail migrate --from src/ --to project.ail.db --verify
```

Add the SQLite working files to `.gitignore`:

```
project.ail.db-wal
project.ail.db-shm
```

Committing `project.ail.db` is a project policy choice: commit for a
queryable graph, ignore to keep the `.ail` tree as the source of truth.

(Optional) Make the SQLite preference explicit in `ail.config.toml`:

```toml
[database]
backend = "auto"
```

`auto` uses SQLite when `project.ail.db` exists next to `ail.config.toml`,
and falls back to the filesystem otherwise. Run the rest of the pipeline
unchanged:

```bash
ail verify
ail build --target python
```

### What `--verify` Checks

`ail migrate --verify` re-parses the source and confirms node, edge, and
contract counts match between the SQLite and filesystem graphs.

### Rollback

Delete `project.ail.db` (and the `-wal` / `-shm` siblings). The filesystem
`.ail` tree is untouched by `ail migrate`.

### Dry-Run Export

Inspect the database by exporting it back to AIL text:

```bash
ail export --from project.ail.db --to exported/
```

Output is a single `exported/export.ail` file, not a per-node tree.

## Opt-In Path 2 — TypeScript Build

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

Run the generated project with Node:

```bash
cd dist-ts
npm install
npx tsc --noEmit
npx vitest run
```

Define types are emitted as branded factories. The factory enforces the
parsed `where` constraint at runtime:

```ts
import { createWalletBalance } from "./types/wallet_balance";

createWalletBalance(10);   // ok
createWalletBalance(-1);   // throws — value >= 0 violated
```

## Opt-In Path 3 — Embedding Search

Requires the `embeddings` crate feature and ONNX model files.

```bash
cargo build -p ail-cli --features embeddings --release
ail search --setup
```

`ail search --setup` checks `~/.ail/models/all-MiniLM-L6-v2/` for the
`tokenizer.json` and `model.onnx` files and prints download guidance if they
are missing.

Once the model is present, build the index for a SQLite-backed project:

```bash
ail reindex --embeddings
```

Then run hybrid search:

```bash
ail search "balance transfer" --semantic
```

Without `--semantic`, `ail search` runs BM25 only. BM25 needs the SQLite
backend (FTS5 index in `project.ail.db`) but not the `embeddings` feature.

## MCP Write Tools (AI Agent Developers)

| Tool         | Purpose                                                                |
|--------------|------------------------------------------------------------------------|
| `ail.write`  | Create a new node (`metadata`, `parent`, `dry_run`).                   |
| `ail.patch`  | Modify an existing node in place (`path`, `value`, `dry_run`).         |
| `ail.move`   | Re-parent or reorder a node (`to_parent`, `dry_run`).                  |
| `ail.delete` | Remove a node (`cascade`, `orphan`, `dry_run`).                        |
| `ail.batch`  | Ordered atomic multi-op; rollback on first error (`dry_run` rejected). |

Write-tool edits live in-memory within a session. `SqliteGraph::save_from_graph()`
is available for explicit disk persistence; the MCP write path does not call
it automatically. `ContextPacket.promoted_facts` is an additive field on the
`ail.context` response; v1.0 clients that ignore unknown fields are unaffected.

## ail.config.toml Changes

A minimal v2.0 config:

```toml
[project]
name = "my_project"
version = "0.1.0"

[build]
target = "python"
contracts = "on"

[database]
backend = "auto"
```

Only `[database] backend` is actively read by the CLI; other fields are
schema placeholders. See `docs/config-reference.md` for the full status table.

## Breaking Changes

None. Non-breaking notes: `AilGraph` API unchanged; pipeline newtypes wrap
`Box<dyn GraphBackend>` with source-compatible signatures; MCP is additive
(5 → 10 tools, none renamed/removed); `ContextPacket.promoted_facts` is
additive; filesystem remains the default backend when `project.ail.db` is
absent.

## Getting Help

- Config field status: `docs/config-reference.md`
- v2.0 command walkthrough: `GETTING_STARTED.md` §§10–14
- End-to-end example: `examples/wallet_service/`
- Per-phase change list: `CHANGELOG.md`

---

# v2.0 → v3.0

## What Changed

- `ail coverage --node NAME` / `--all` / `--warm-cache` / `--from-db PATH`:
  semantic coverage scorer reporting **Full / Partial / Weak / N/A** with
  missing-aspect hints.
- `ail agent "<task>"`: LangGraph-driven agent (Python) that plans, writes
  nodes via the MCP write tools, and re-verifies the graph. Five provider
  adapters — `anthropic`, `openai`, `deepseek`, `alibaba` (alias `qwen`),
  `ollama`.
- New MCP tool `ail.review` returns coverage and missing-aspect data for a
  named target node.
- New `[coverage]` and `[agent]` sections in `ail.config.toml` (both
  ACTIVE — see `docs/config-reference.md`).
- `StatusOutput.root_id: Option<String>` — additive field on the existing
  `ail.status` MCP response, consumed by the agent's coder.

## No-Change Path

Every v2.0 surface is preserved:

- `.ail` grammar and all 17 syntax patterns.
- `ail init`, `ail verify`, `ail build` (Python and TypeScript), `ail test`,
  `ail search`, `ail context`, `ail status`, `ail migrate`, `ail export`,
  `ail serve`.
- MCP read tools (5) and write tools (5) — no rename, no removal.
- SQLite + filesystem backends, `[database] backend = "auto"` resolution,
  `--from-db` flag.
- CIC propagation rules (down / up / across / diagonal) and path-sensitive
  promoted facts.
- `examples/wallet_service/` pipeline is unchanged.

## Opt-In Paths

### 1. Enable semantic coverage

Add `[coverage]` to `ail.config.toml` (all fields optional — the section
header alone is enough because defaults are sensible):

```toml
[coverage]
enabled = true
threshold_full = 0.8
threshold_partial = 0.5
extra_concepts = ["error handling", "observability"]
```

Run `ail coverage --all` from the project root. Feature-gated by the
`embeddings` Cargo feature; when absent, the section is accepted but
scoring is skipped with a diagnostic.

### 2. Enable the AI agent

```bash
pip install ./agents/
export ANTHROPIC_API_KEY=sk-...
cd examples/wallet_service
ail agent "add error handling to transfer_money"
```

See `agents/README.md` for the full provider table, API-key environment
variables, and troubleshooting. Optional provider extras:
`pip install './agents/[openai]'` / `[deepseek]` / `[alibaba]` /
`[ollama]` / `[all]`.

### 3. Configure agent defaults via `[agent]` TOML

```toml
[agent]
model = "openai:gpt-4o"
max_iterations = 100
steps_per_plan = 30
```

CLI flags on `ail agent` override these values. When neither is set, the
Python side falls back to `anthropic:claude-sonnet-4-5`, `50`, and `20`.

## Breaking Changes

None. All additions are opt-in:

- `[coverage]` and `[agent]` TOML sections are optional.
- `StatusOutput.root_id` uses serde `skip_serializing_if` — v2.0 MCP
  clients that ignore unknown fields are unaffected.
- `Command::Agent` is a new subcommand; existing CLI invocations keep
  working.

## Getting Help

- Config field status: `docs/config-reference.md`
- Agent install + provider guide: `agents/README.md`
- Integration reference spec: `docs/plan/v3.0/reference/AIL-Integration-Release-v3.0.md`
- Per-phase change list: `CHANGELOG.md`
