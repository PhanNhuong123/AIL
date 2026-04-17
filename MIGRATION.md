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
