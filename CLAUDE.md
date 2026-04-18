# CLAUDE.md — AIL Codebase Guide

AIL (AI Language) is a Rust workspace compiler for a domain-specific language
designed for AI-driven code generation with formal verification. The primary use
case: AI writes structured English intent (`.ail` files), the compiler verifies
constraints with Z3, then emits production-ready Python or TypeScript.

---

## Architecture

```
.ail files → Parse → PSSD Graph → CIC Packets → Z3 Verify → Emit → Python/TS + Tests
                          ↓                                            ↑
                   .ail.db (SQLite)                              dist/ or dist-ts/
                 BM25 + embeddings
```

### Crate Map

| Crate | Role |
|-------|------|
| `ail-text` | Pest PEG parser + deterministic `.ail` renderer |
| `ail-graph` | PSSD graph (petgraph), CIC packet engine, BM25 full-text, `CoverageInfo` |
| `ail-types` | Constraint AST, type checker, `TypedGraph` |
| `ail-contract` | Static checks, Z3 formal verification (`z3-verify` feature), `VerifiedGraph` |
| `ail-db` | SQLite backend (`SqliteGraph`), FTS5 BM25, CIC cache, coverage cache, embedding vectors |
| `ail-emit` | Python emitter (pytest stubs, contracts, source maps) + TypeScript emitter (Vitest stubs, branded factories) |
| `ail-search` | `EmbeddingProvider` trait, `OnnxEmbeddingProvider`, hybrid RRF search (`embeddings` feature) |
| `ail-coverage` | Semantic coverage scoring: `compute_coverage`, `CoverageResult`, missing-aspect detection |
| `ail-mcp` | MCP server over stdio — 11 tools (5 read + 5 write + `ail.review`) |
| `ail-cli` | `ail` binary — all CLI commands via `clap` |

---

## Build & Test

```bash
# Standard build (no Z3, no embeddings)
cargo build

# With Z3 formal verification
cargo build -p ail-contract --features z3-verify

# With ONNX embedding search + coverage
cargo build -p ail-cli --features embeddings

# Run all tests
cargo test

# Run with Z3 feature
cargo test -p ail-contract --features z3-verify

# Lint / format
cargo clippy -- -D warnings
cargo fmt --all -- --check
```

CI runs on Ubuntu and Windows. Z3 requires `cmake`, `clang`, `libclang-dev` on
Linux or LLVM on Windows. The `embeddings` feature requires ONNX model files at
`~/.ail/models/all-MiniLM-L6-v2/`.

---

## CLI Commands

```bash
# Project lifecycle
ail init <name>                           # scaffold new project
ail build [--target python|typescript]    # full pipeline → generated source
ail build --watch                         # rebuild on file change
ail build --contracts on|comments|off     # contract emission mode
ail build --source-map                    # write functions.ailmap.json
ail build --from-db <path>                # build from specific .ail.db
ail verify [file] [--from-db <path>]      # pipeline without emitting
ail test                                  # build + run pytest
ail status                                # pipeline stage + graph stats

# Migration / export
ail migrate --from <src/> --to <db> [--verify]
ail export --from <db> --to <dir>

# Search
ail search <query>                        # BM25 full-text (requires SQLite)
ail search <query> --semantic             # hybrid RRF (requires embeddings feature)
ail search <query> --bm25-only
ail search <query> --budget <n>
ail search --setup                        # check ONNX model files
ail reindex                               # clear embedding vectors
ail reindex --embeddings                  # rebuild embedding index

# CIC context
ail context --task "<text>" [--from-db <path>]
ail context --node <name>  [--from-db <path>]

# Coverage (Phase 13 — requires SQLite + embeddings feature)
ail coverage --node <name>    # coverage for one node (cache-aware)
ail coverage --all            # summary across all non-leaf nodes
ail coverage --warm-cache     # recompute all nodes, persist to cache

# MCP server
ail serve                                 # start MCP server over stdio
```

---

## MCP Tools (11 total)

Connect AI tools (Claude, Cursor) via:
```json
{ "mcpServers": { "ail": { "command": "ail", "args": ["serve"] } } }
```

**Read (5):** `ail.search`, `ail.context`, `ail.verify`, `ail.build`, `ail.status`

**Write (5):** `ail.write`, `ail.patch`, `ail.move`, `ail.delete`, `ail.batch`
- Write tools support `cascade`, `orphan`, and `dry_run` semantics
- `ail.batch` is atomic with rollback; `dry_run` is rejected inside a batch
- All write tools run auto-edge detection and demote context to `Raw`, clearing BM25/embedding caches

**Review (1):** `ail.review`
- Semantic coverage review for a single graph node (requires `embeddings` feature)
- Returns `CoverageStatus`, `ChildContribution` list, missing aspects, and a human-readable suggestion
- Returns a structured `"Unavailable"` response (not an error) when embeddings are disabled

---

## Key Concepts

**PSSD Graph** — directed graph with three edge types:
- `Ev` (vertical): parent → child decomposition
- `Eh` (horizontal): sibling → sibling sequence
- `Ed` (diagonal): cross-references (function ↔ type ↔ error)

**CIC — Constraint Inheritance Chain** — four propagation rules:
1. DOWN: parent constraints → all descendants
2. UP: verified child postcondition → parent fact
3. ACROSS: previous sibling output → next sibling
4. DIAGONAL: type constraints → auto-inject everywhere the type is used

**Coverage** (`ail-coverage`, Phase 13):
- `compute_coverage(graph, parent_node, provider)` computes semantic similarity
  between a parent node's intent and each child's contribution
- `CoverageStatus`: `Full` / `Partial` / `Sparse`
- `MissingAspect`: concepts from a default list not addressed by any child
- Results are cached in the `coverage_cache` SQLite table; invalidated on graph writes

---

## Backend Resolution

CLI commands resolve the storage backend in this order:
1. `--from-db <path>` flag (explicit)
2. `[database] backend` in `ail.config.toml` (`"sqlite"` | `"filesystem"` | `"auto"`)
3. Auto-detect: if `project.ail.db` exists alongside `ail.config.toml` → SQLite
4. Fallback: filesystem

`coverage` and `context` commands require the SQLite backend.

---

## Project Status

| Phase | Deliverable | State |
|-------|-------------|-------|
| 1 | `ail-graph` — PSSD graph, CIC packets, BM25, validation | Done |
| 2 | `ail-types` — constraint AST, type checker, `TypedGraph` | Done |
| 3 | `ail-contract` — static checks, Z3 verification, `VerifiedGraph` | Done |
| 4 | `ail-text` — pest parser, deterministic `.ail` renderer | Done |
| 5a | `ail-emit` — Python emitter, pytest stubs, source maps | Done |
| 5b | `ail-mcp` + `ail-cli` — MCP server, `clap` CLI | Done |
| 6 | v1.0 polish — `wallet_service` fixture, docs, quality gates | Done |
| 7 | `ail-db` — SQLite backend, FTS5, CIC cache, `ail migrate`/`ail export` | Done |
| 8 | Path-sensitive CIC — check promotion, `promoted_facts`, `ail context` | Done |
| 9 | TypeScript emitter — `dist-ts/`, branded factories, Vitest stubs | Done |
| 10 | `ail-search` — embedding provider, hybrid RRF, `ail reindex` | Done |
| 11 | MCP write tools — `write`, `patch`, `move`, `delete`, `batch` (atomic) | Done |
| 12 | Integration release — `wallet_service` e2e Python + TypeScript | Done |
| 13 | `ail-coverage` — semantic coverage scoring, `ail coverage` CLI, `ail.review` MCP tool, coverage cache in `ail-db` | Done |

Current version: **v2.0.0** (Unreleased — all phases complete).

---

## Known Limitations

- `z3-verify` feature: 8 tests in `crates/ail-mcp/tests/ai_workflow.rs` fail on
  the `wallet_full` fixture (AIL-C012: postcondition not entailed because fixture
  lacks `amount <= sender.balance`). Default builds unaffected.
- `ail search --semantic` and `ail coverage` require `--features embeddings` and
  ONNX model files at `~/.ail/models/all-MiniLM-L6-v2/`.
- MCP write-tool edits persist in-memory within a session; `SqliteGraph::save_from_graph()`
  must be called explicitly for disk persistence.
- `ail context` and `ail coverage` require the SQLite backend.
- `ail export` writes a single `export.ail`, not a per-node file tree.

---

## Example Project

`examples/wallet_service/` — canonical end-to-end example (flat `src/` layout,
SQLite backend). Run with:

```bash
cd examples/wallet_service
ail build --target python
ail test
ail build --target typescript
```

End-to-end tests:
- `crates/ail-cli/tests/cli_e2e_wallet_sqlite.rs`
- `crates/ail-cli/tests/cli_e2e_wallet_ts.rs`
