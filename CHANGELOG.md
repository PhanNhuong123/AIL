# Changelog

All notable changes to this project are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Unreleased changes are under the [Unreleased] heading.

## [Unreleased]

## [3.0.0] - 2026-04-20

v3.0 turns AIL from a verified compiler into a verified agent loop. Three
pillars land together: the SQLite graph backend (carried forward from v2.0),
**semantic coverage** scoring via `ail coverage`, and the **LangGraph AI
agent** that plans, writes, and re-verifies through the MCP write tools. The
hard Rust pipeline — `.ail → AilGraph → ValidGraph → TypedGraph →
VerifiedGraph → emit` — is unchanged. Every new surface is additive.

### Added

#### Phase 13 — Semantic Coverage

- `ail coverage --node NAME` / `--all` / `--warm-cache` / `--from-db PATH`
  computes intent-vs-child semantic coverage and prints Full / Partial /
  Weak / N/A with missing-aspect hints. The `--all` output includes a
  `StatusCounts` summary across every non-leaf node.
- `CoverageStatus` enum (`Full`, `Partial`, `Weak`, `N/A`, `Leaf`,
  `Unavailable`) and `CoverageResult` / `CoverageInfo` public types in
  `ail-graph::coverage`.
- `[coverage]` section in `ail.config.toml` (hand-written line scanner):
  `enabled`, `threshold_full`, `threshold_partial`, `extra_concepts`.
- `invalidate_coverage_for_ancestors` hook in `ail-db` clears
  `coverage_cache` rows for every ancestor when a child node is written
  through `update_node` or MCP write tools — so warmed scores stay fresh
  after agent writes.
- MCP tool `ail.review` returns coverage and missing-aspect data for a
  named target node without running the full Z3 pipeline.

#### Phase 14 — AI Agent Foundation

- New `agents/` Python package (`python -m ail_agent <task>` and the
  `ail-agent` console script) built on **LangGraph 1.1.8**. JSON-safe
  `AILAgentState`, deterministic router, iteration guard (AIL-G0142), and a
  plan → code → verify → done graph.
- Five provider adapters behind a lazy registry: `anthropic`, `openai`,
  `deepseek`, `alibaba` (alias `qwen`), `ollama`. Provider selection uses
  `provider:model` specs; SDKs are optional extras.
- Common `LLMProvider` interface: `ToolCall`, `ToolSpec`, `CompletionResult`
  with normalised tool-call results and retry helper.
- `planner.run_planner` (strict JSON plan parser — rejects `"keep"` contract
  kind to match Rust `ContractKind`), `coder.run_coder` (resolves
  `parent_id` from `"root"`, label, or UUID; enforces the two-check budget
  guard — plan-complete → verify, budget-exceeded → `AIL-G0143`), and
  `verify.run_verify` (lightweight `ail.status` sanity check + `VERIFY_OK`
  follow-up line).
- `MCPToolkit`: synchronous facade over the `mcp==1.9.2` async SDK via a
  dedicated event-loop thread. Raises `MCPConnectionError` (AIL-G0145).
- New CLI command `ail agent <task>` in `ail-cli` subprocess-spawns
  `python -m ail_agent` with `Stdio::inherit()`. Accepts `--model`,
  `--mcp-port` (default 7777), `--max-iterations`, `--steps-per-plan`.
- **`[agent]` TOML section parser** (`read_agent_config` in
  `crates/ail-cli/src/commands/agent.rs`): fields `model`,
  `max_iterations`, `steps_per_plan`. `run_agent` applies 3-way precedence
  — CLI flag → `[agent]` TOML value → Python-side default.
- New agent errors: `AgentError` / `ProviderError` / `ProviderConfigError`
  (G0140), `RoutingError` (G0141), `StepBudgetError` (G0143),
  `PlanError` (G0144), `MCPConnectionError` (G0145).

#### Phase 15 — Integration + Release

- E2E coverage regression on `examples/wallet_service/`:
  `crates/ail-cli/tests/cli_e2e_wallet_coverage.rs` and
  `crates/ail-mcp/tests/review_tool_e2e_wallet.rs` lock `cmd_all`, leaf
  tally, cache invalidation, and `ail.review` response shape (task 15.1).
- Agent E2E on `wallet_service`: `agents/tests/test_workflow_e2e_wallet.py`,
  `crates/ail-mcp/tests/agent_e2e_wallet.rs`, and
  `crates/ail-cli/tests/cli_e2e_wallet_agent.rs` lock a 3-step
  error-handling plan reaching `done`, MCP `ail.status → ail.write × 3 →
  ail.status` sequencing, and `ail.verify` parity after agent writes
  (task 15.2).
- Provider swap E2E: `agents/tests/test_provider_swap.py` covers all 5
  providers + `qwen` alias, missing API keys surfacing as workflow
  error (exit 1), malformed specs surfacing as CLI error (exit 2), and
  default model lock. New `cli_agent.rs` argv-forwarding tests prove
  `--model` propagates to the subprocess only when set (task 15.3).
- Release polish: `docs/config-reference.md` `[agent]` ACTIVE section,
  expanded `agents/README.md` (install, providers, exit codes,
  troubleshooting), `agents/Makefile` (`install-agents`, `test`,
  `clean`, `help`), and the release-procedure block below (task 15.4).

### Changed

- `StatusOutput.root_id: Option<String>` (serde `skip_serializing_if`):
  additive field consumed by the coder to resolve the `"root"` literal in
  planner output.
- `Command::Agent` added to the `ail-cli` clap tree; help output for
  `ail agent` now renders `--mcp-port` default (7777). Other flags stay
  `Option<…>` so 3-way precedence (CLI → `[agent]` TOML → Python default)
  keeps working.
- `pip install ./agents/[anthropic]` (or `[openai]`, `[deepseek]`,
  `[alibaba]`, `[ollama]`, `[all]`) selects provider SDK extras; base
  dependencies pin `langgraph==1.1.8`, `langchain-core==1.3.0`,
  `langchain-mcp-adapters==0.2.2`, `mcp==1.9.2`, `typing-extensions==4.14.0`.

### Fixed

- `ail-contract` Z3 encoder now encodes `Bool` operand comparisons
  (commit `3fcbbb4`). Previously, boolean-typed operands in comparisons
  were not emitted to the SMT backend, causing unsound accept paths on
  contracts that combined bool equality with arithmetic predicates.

### Known Issues / Limitations

- The coder has no name-based `parent_id` resolver: planner output must
  reference `"root"`, an exact label already in `node_id_map`, or a raw
  UUID. Fuzzy or symbolic lookups are tracked for v3.1+.
- `OllamaProvider` has no preflight API-key / base-URL health check; a
  failing local daemon surfaces only after the first request.
- `deepseek:deepseek-reasoner` and `qwen:qwen-turbo` use the
  prompt-fallback path rather than native tool-calling, which is slower
  and more error-prone than the first-class models.
- `[agent] timeout_seconds` is **not yet parsed** — the Python side has
  no corresponding config hook. The key is documented but silently
  ignored. Tracked for v3.1+.

## [2.0.0] - 2026-04-17

v2.0 introduces a SQLite graph backend, a TypeScript emitter, embedding-based
hybrid search, MCP write tools for AI agents, and path-sensitive CIC context
packets. The filesystem `.ail` workflow is preserved; v2.0 features are opt-in.

### Added

#### Phase 7 — SQLite Backend

- `ail migrate --from <src/> --to <project.ail.db> [--verify]` migrates a
  filesystem AIL project into a single `.ail.db` SQLite database.
- `ail export --from <db> --to <dir>` exports a `.ail.db` back to AIL text
  (single `export.ail` file).
- `SqliteGraph` in `ail-db` with the `GraphBackend` trait; auto-detected when
  `project.ail.db` exists alongside `ail.config.toml`.
- `[database] backend = "auto" | "sqlite" | "filesystem"` in `ail.config.toml`
  is the only configuration key actively parsed by the CLI at runtime.
- `--from-db <path>` flag on `ail build` and `ail verify` to force a specific
  database file regardless of project layout.
- FTS5 BM25 full-text search index on the `nodes` table and CLI command
  `ail search <query>`.
- `cic_cache` table for context-packet reuse across CLI invocations.

#### Phase 8 — Path-Sensitive CIC

- `ail context --task <text>` / `--node <name>` subcommand prints the CIC
  context packet for the chosen target. Requires the SQLite backend.
- `ContextPacket.promoted_facts` field (additive) — facts proven on the
  prevailing path are exposed to downstream consumers (CLI and MCP).
- `check` promotion in the CIC engine: when a node's `check` succeeds on a
  path, the success condition is promoted to a fact for descendants on that
  path.

#### Phase 9 — TypeScript Emitter

- `ail build --target typescript` emits a TypeScript project to `dist-ts/`.
- Strict `tsconfig.json`, `package.json`, and an inlined `ail-runtime.ts`
  are written alongside per-feature `types/`, `errors/`, `fn/`, and `tests/`
  folders.
- Branded factory types enforce define constraints at runtime
  (e.g. `createWalletBalance(-1)` throws).
- Vitest test stubs use hardcoded v2.0 fixture values for known paths,
  `it.skip` for error paths, and `it.todo` for boundary cases.
- Top-level `do` functions are exported. Private nested helper functions are
  not exported.

#### Phase 10 — Embedding Search

- New `ail-search` crate with the `EmbeddingProvider` trait,
  `OnnxEmbeddingProvider`, `EmbeddingIndex`, and `hybrid_search` (Reciprocal
  Rank Fusion of BM25 and semantic results).
- `HybridSearchResult` and `RankingSource` carry per-result provenance
  metadata.
- `ail search --semantic` (requires the `embeddings` crate feature) runs the
  hybrid RRF path. `ail search --bm25-only` forces keyword-only search even
  when `--semantic` is also passed.
- `ail search --budget <n>` caps result count; `ail search --setup` checks
  for ONNX model files at `~/.ail/models/all-MiniLM-L6-v2/` and prints setup
  guidance.
- `ail reindex` clears persisted embedding vectors; `ail reindex --embeddings`
  rebuilds the index using the configured provider.
- `ail status` includes an embedding health line: model name, provider,
  dimensions, and per-node coverage.
- `embeddings` table in SQLite stores model name, provider, dimensions, and
  index version; mismatches block read paths until the index is rebuilt.

#### Phase 11 — MCP Write Tools

- MCP tool count expanded from 5 to 10. The new write tools are `ail.write`,
  `ail.patch`, `ail.move`, and `ail.delete` (each supports `cascade`,
  `orphan`, and `dry_run` semantics where applicable), plus `ail.batch`
  for ordered atomic multi-op transactions.
- `ail.batch` uses an in-memory `AilGraph::clone` snapshot for rollback on
  the first failed op and runs a post-batch auto-edge reconciliation pass
  over every touched node. `dry_run` is rejected inside a batch.
- All write tools run auto-edge detection over `node.expression` and
  `node.intent` (PascalCase type/error references, snake_case `do`
  function calls, `raise` and `otherwise raise` error references, and
  `follows_template` from metadata).
- Every mutating tool demotes `ProjectContext` to `Raw` and clears the BM25
  and embedding caches, so the next `ail.verify` or `ail.build` re-derives
  facts from the live graph instead of the on-disk snapshot.
- `ail.verify` and `ail.build` are dirty-aware: after writes they refresh
  through `pipeline::refresh_from_graph(AilGraph)` (no disk re-parse).
- `ContextPacket.promoted_facts` is exposed in the `ail.context` MCP
  response.

#### Phase 12 — Integration Release

- `examples/wallet_service/` is the canonical end-to-end example. Flat
  `src/` layout, `ail.config.toml` declaring `[database] backend = "auto"`.
- `ail-cli` backend resolver: `commands/project.rs::resolve_backend()` and
  `load_graph()` with precedence `--from-db` → `[database] backend` →
  `auto` with `project.ail.db` presence check → filesystem.
- `SqliteGraph::save_from_graph(&mut self, &AilGraph)` performs an in-order
  drop-and-reinsert write-back (resolves the Phase 11 SQLite write-back
  deferred item).
- New integration suites
  `crates/ail-cli/tests/cli_e2e_wallet_sqlite.rs` and
  `crates/ail-cli/tests/cli_e2e_wallet_ts.rs` lock the wallet_service
  pipeline on both backends and both emission targets.
- User-facing v2.0 documentation: `MIGRATION.md`, `CHANGELOG.md`,
  `docs/config-reference.md`, refreshed `README.md`,
  `GETTING_STARTED.md`, and `examples/wallet_service/README.md`.

### Changed

- `ail build` and `ail verify` route through `resolve_backend()` for both
  Python and TypeScript targets (no callers reach `parse_directory`
  directly).
- CIC context packets carry the additive `promoted_facts` field. v1.0
  consumers that ignore unknown fields are unaffected.
- The MCP server is dirty-aware: write-tool calls mark the in-memory
  context dirty and `ail.verify` / `ail.build` re-pipeline from the
  in-memory graph rather than re-parsing from disk.

### Fixed

- Template-edge false-positive: graph rule v008 and contract check C005 no
  longer trip on auto-generated Ed edges produced by ordinary
  type/error/function references. Both checks now follow templates only via
  `metadata.following_template_name` + `find_by_name`.
- `detect_auto_edges` extended scan: scans both `node.expression` and
  `node.intent`, extracts snake_case `do` function calls and `raise` /
  `otherwise raise` error references, and emits `follows_template` from
  metadata.

### Known Issues / Limitations

- With the optional `z3-verify` feature
  (`cargo build --features z3-verify`), 8 pre-existing tests in
  `crates/ail-mcp/tests/ai_workflow.rs` fail on the `wallet_full` fixture
  with `AIL-C012 postcondition 'balance >= 0' is not entailed`. Root cause:
  the fixture does not constrain `amount <= sender.balance`. Default
  builds are unaffected because z3-verify is not a default feature.
- `ail search --semantic` requires the `embeddings` crate feature and ONNX
  model files at `~/.ail/models/all-MiniLM-L6-v2/`. The standard release
  binary excludes this; build with
  `cargo build -p ail-cli --features embeddings`.
- MCP write-tool edits persist in-memory within a session.
  `SqliteGraph::save_from_graph()` is available but is not called
  automatically from the MCP write path; explicit invocation is required
  for disk persistence.
- `ail context` requires the SQLite backend. Filesystem-only projects
  return an error with a remediation hint.
- `ail export` writes a single `export.ail` file, not a per-node tree.

## [1.0.0] - 2026-04-15

### Added

- Full v1.0 pipeline:
  parse (`ail-text`, pest grammar) → validate (`ail-graph`) →
  type-check (`ail-types`) → Z3 contract verification (`ail-contract`) →
  Python emission with pytest stubs (`ail-emit`) →
  MCP server over stdio (`ail-mcp`) → CLI (`ail-cli`).
- Five MCP tools: `ail.search`, `ail.context`, `ail.verify`, `ail.build`,
  `ail.status`.
- CIC (Constraint Inheritance Chain) with four propagation rules:
  down (parent → descendants), up (verified child postcondition becomes
  parent fact), across (previous sibling output → next sibling),
  and diagonal (type constraints auto-inject everywhere the type is used).
- CLI commands: `ail init`, `ail build`, `ail verify`, `ail test`,
  `ail serve`, `ail status`.
- `wallet_service` fixture with 196 passing tests end-to-end.

## Release Procedure — v3.0.0

The following sequence documents the v3.0.0 release.

```
1. cargo test --workspace                                 (must pass)
2. python -m pytest agents/tests/                         (must pass)
3. cargo build --release                                  (must pass)
4. Clean-venv install smoke test:
     python -m venv .venv-v3
     .venv-v3/Scripts/pip install ./agents/               (Windows/Git Bash)
     .venv-v3/Scripts/ail-agent --help                    (exit 0 required)
     rm -rf .venv-v3
5. git tag -a v3.0.0 -m "AIL v3.0.0 — Semantic coverage + AI agent foundation"
6. git push origin v3.0.0
7. git push origin master
8. (Optional) Create a GitHub release from the v3.0.0 tag using this
   CHANGELOG section as release notes.
```

## Release Procedure — v2.0.0

The following sequence documents the v2.0.0 release.

```
1. cargo test --workspace                                 (must pass)
2. cargo build --workspace                                (must pass)
3. git tag -a v2.0.0 -m "AIL v2.0.0 — SQLite backend, TypeScript emitter, embedding search, MCP write tools, path-sensitive CIC"
4. git push origin v2.0.0
5. git push origin master
```

[Unreleased]: https://github.com/PhanNhuong123/AIL/compare/v3.0.0...HEAD
[3.0.0]: https://github.com/PhanNhuong123/AIL/compare/v2.0.0...v3.0.0
[2.0.0]: https://github.com/PhanNhuong123/AIL/compare/v1.0.0...v2.0.0
[1.0.0]: https://github.com/PhanNhuong123/AIL/releases/tag/v1.0.0
