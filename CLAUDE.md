# CLAUDE.md — AIL Codebase Guide

## What Is AIL

AIL (AI Language) is a Rust-implemented compiler and toolchain for a structured-English DSL where **code = document = constraint**. AI agents write specs in `.ail` files; the toolchain verifies them with Z3, then emits production Python or TypeScript. The core insight is CIC (Constraint Inheritance Chain): constraints declared at any depth are automatically propagated to all child nodes so AI never works without context.

## Repository Layout

```
Cargo.toml                  ← Rust workspace (resolver = "2")
crates/
  ail-cli/                  ← Binary entry-point + all CLI commands (clap)
  ail-graph/                ← PSSD graph, node/edge types, BM25 search, CIC engine
  ail-db/                   ← SQLite backend (rusqlite, FTS5, CIC cache, embeddings, coverage)
  ail-types/                ← Type system and constraint AST
  ail-contract/             ← Z3 formal verification (feature-gated: z3-verify)
  ail-text/                 ← PEG parser (pest) and renderer
  ail-emit/                 ← Python and TypeScript code generators
  ail-mcp/                  ← MCP server (JSON-RPC 2.0 over stdio, 11 tools)
  ail-search/               ← Hybrid BM25 + ONNX embedding search (RRF fusion)
  ail-coverage/             ← Semantic coverage scoring
examples/
  wallet_service/           ← End-to-end test fixture (.ail source files)
```

## Core Concepts

### Patterns (17 total — closed vocabulary)

| Category | Patterns |
|----------|----------|
| Type-defining | `Define`, `Describe`, `Error` |
| Structural | `Do`, `Promise` |
| Action (leaf only) | `Let`, `Check`, `ForEach`, `Match`, `Fetch`, `Save`, `Update`, `Remove`, `Return`, `Raise`, `Together`, `Retry` |

Unknown patterns are a validation error. Serialised as `snake_case` in JSON/SQLite.

### Edge Kinds

- **Ev** — Vertical: parent → child decomposition (structural nesting)
- **Eh** — Horizontal: sibling → sibling sequence (execution order)
- **Ed** — Diagonal: cross-reference (type, error, function, template). Auto-detected from PascalCase references and `do`/`raise` keywords in expressions.

### Node Invariants (enforced by `ValidGraph`)

- `intent` must be non-empty.
- Structural nodes (`children.is_some()`) must have `expression == None`.
- Leaf nodes (`children.is_none()`) may carry an `expression`.
- `Do` nodes must have at least one `Before` and one `After` contract.

### CIC — Constraint Inheritance Chain

CIC packets are computed in `ail-graph/src/cic/`. Each packet contains all constraints (promises, type constraints, scope variables, promoted facts from `check … otherwise raise`) that apply at a given node's position in the graph. Packets are cached in SQLite (`cic_cache` table) and invalidated on graph mutation.

### Pipeline Stages

```
.ail files (or SQLite DB)
    ↓  ail-text  (PEG parser, pest)
Raw AilGraph
    ↓  ail-types  (type checker)
Typed AilGraph
    ↓  ail-contract  (Z3, feature z3-verify)
ValidGraph
    ↓  ail-emit
Python / TypeScript files + test stubs
```

The `ProjectContext` enum in `ail-mcp` tracks pipeline stage. Write tools mark `dirty = true`; subsequent verify/build calls use `refresh_from_graph` (preserve edits) rather than `refresh_from_path` (re-parse disk).

## CLI Commands

Run via `cargo run -p ail-cli -- <command>` or the installed `ail` binary.

| Command | Description |
|---------|-------------|
| `init <name>` | Scaffold a new AIL project |
| `build [--watch] [--target python\|typescript] [--contracts on\|comments\|off] [--source-map] [--from-db PATH]` | Parse → verify → emit |
| `verify [FILE] [--from-db PATH]` | Run pipeline without emitting |
| `context [--task TEXT] [--node NAME] [--from-db PATH]` | Print CIC context packet |
| `test` | Build then run generated pytest tests |
| `run` | Build and run the project entry point |
| `serve` | Start MCP server over stdio |
| `status` | Show pipeline stage + node/edge counts |
| `search [QUERY] [--budget N] [--semantic] [--bm25-only] [--setup]` | Semantic/BM25 search |
| `reindex [--embeddings]` | Rebuild the embedding index |
| `migrate --from DIR --to PATH [--verify]` | Migrate filesystem project → SQLite DB |
| `export --from PATH --to DIR` | Export SQLite DB → `.ail` text file |
| `coverage [--node NAME] [--all] [--warm-cache] [--from-db PATH]` | Semantic coverage scoring |

## MCP Tools (11 tools, JSON-RPC 2.0 over stdio)

Start with `ail serve` (or `ail-cli serve`). Protocol version: `2024-11-05`.

### Read tools

| Tool | Key inputs | Description |
|------|-----------|-------------|
| `ail.search` | `query`, `budget` | BM25 + optional semantic (RRF fusion) over graph nodes |
| `ail.review` | `node_id`, (optional) provider | Semantic review of a node against its CIC context |
| `ail.context` | `task`, `budget_tokens` | CIC context packet for a task description |
| `ail.verify` | (none / options) | Run full pipeline and return errors |
| `ail.build` | `async_mode`, `contracts` | Emit Python files; returns list of generated files |
| `ail.status` | (none) | Pipeline stage, node count, edge count |

### Write tools

| Tool | Key inputs | Description |
|------|-----------|-------------|
| `ail.write` | `parent_id`, `pattern`, `intent`, `expression?`, `contracts?`, `position?`, `metadata?` | Create a new node; auto-detects Ed edges |
| `ail.patch` | `node_id`, `fields` | Update fields on an existing node; diffs Ed edges |
| `ail.move` | (node_id, target position) | Reposition a node in the sibling chain |
| `ail.delete` | `node_id`, `strategy` | Remove a node (`dry_run` strategy available) |
| `ail.batch` | list of operations | Atomic batch of write/patch/move/delete; rolls back on failure |

All write tools set `dirty = true` and clear BM25/embedding caches. On error, `ail.batch` restores the graph snapshot.

## Development Workflow

```bash
# Build all crates (no features)
cargo build

# Build with Z3 verification
cargo build -p ail-contract --features z3-verify

# Run all tests
cargo test

# Run Z3 tests
cargo test -p ail-contract --features z3-verify

# Lint (zero warnings enforced in CI)
cargo clippy -- -D warnings

# Format check
cargo fmt --all -- --check

# Format fix
cargo fmt --all
```

CI runs on Ubuntu and Windows. Ubuntu requires `cmake clang libclang-dev` for Z3 static linking. Windows requires LLVM (`choco install llvm`) with `LIBCLANG_PATH` set.

## SQLite Backend (ail-db)

The `.ail.db` file stores:
- All graph nodes and edges (JSON-serialised via `rusqlite`)
- FTS5 full-text search index
- CIC context packet cache (`cic_cache` table)
- Embedding vectors (float32, ONNX model)
- Coverage scores (`coverage` table)

Use `--from-db PATH` on CLI commands or `migrate`/`export` to switch between filesystem and DB modes.

## Embedding / Semantic Search

ONNX model files must be placed at `~/.ail/models/`. Run `ail search --setup` to verify. The `ail-search` crate wraps `ort` (ONNX Runtime); feature flag `embeddings` is required. Hybrid search uses Reciprocal Rank Fusion (RRF, k=60) over BM25 and semantic rankings.

## Example Project

`examples/wallet_service/` contains `.ail` sources (`transfer_money.ail`, `add_money.ail`, etc.) that exercise the full pipeline. Use it as a reference for `.ail` syntax and to run integration tests.

## Key File Locations

| What | Where |
|------|-------|
| CLI command routing | `crates/ail-cli/src/lib.rs` |
| All CLI command impls | `crates/ail-cli/src/commands/` |
| Node / Pattern / EdgeKind types | `crates/ail-graph/src/types/` |
| CIC computation | `crates/ail-graph/src/cic/` |
| Graph backend trait + impls | `crates/ail-graph/src/graph/` |
| MCP server dispatch | `crates/ail-mcp/src/server.rs` |
| MCP tool I/O types | `crates/ail-mcp/src/types/tool_io.rs` |
| MCP tool handlers | `crates/ail-mcp/src/tools/` |
| Python emitter | `crates/ail-emit/src/python/` |
| TypeScript emitter | `crates/ail-emit/src/typescript/` |
| SQLite schema + impls | `crates/ail-db/src/db/` |
| Hybrid search | `crates/ail-search/src/hybrid.rs` |
| Coverage computation | `crates/ail-coverage/src/coverage.rs` |
| CI configuration | `.github/workflows/ci.yml` |
