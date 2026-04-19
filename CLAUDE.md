# CLAUDE.md — AIL Project Guide

## Project Overview

AIL (AI Layer) is a Rust + Python verification framework that sits between AI agents and generated code. It enforces domain constraints via Z3 formal proofs, preventing AI from silently breaking invariants across context windows.

**Architecture:** Rust core (graph + Z3 + parser + emitter) + Python agent layer (LangGraph + MCP client).

**Current phase:** v3.0, Phase 15 (agent E2E tests).

**License:** BUSL-1.1 core (→ MIT 2030-04-19); MIT for generated runtime code (`crates/ail-runtime-py/`).

---

## Repository Layout

```
/
├── crates/                  # Rust workspace (10 crates)
│   ├── ail-graph/           # PSSD graph + CIC engine + BM25 index
│   ├── ail-types/           # Type system + constraint expressions
│   ├── ail-contract/        # Z3 encoding + verification pipeline
│   ├── ail-text/            # PEG parser (pest)
│   ├── ail-emit/            # Python / TypeScript / Rust code generators
│   ├── ail-cli/             # CLI binary (14 subcommands)
│   ├── ail-mcp/             # MCP server (10 JSON-RPC tools)
│   ├── ail-db/              # SQLite persistence
│   ├── ail-search/          # BM25 + ONNX embeddings
│   ├── ail-coverage/        # Semantic coverage (SCFT)
│   └── ail-runtime-py/      # MIT-licensed Python runtime validators
├── agents/                  # Python LangGraph agent layer
│   ├── ail_agent/
│   │   ├── orchestrator.py  # LangGraph state machine + AILAgentState
│   │   ├── planner.py       # LLM → structured plan steps
│   │   ├── coder.py         # Plan step → MCP ail.write
│   │   ├── verify.py        # Post-code ail.status sanity check
│   │   ├── mcp_toolkit.py   # Sync wrapper over async MCP SDK
│   │   ├── prompts.py       # System + user prompt templates
│   │   ├── plan_format.py   # PlanStep TypedDict + parser
│   │   ├── errors.py        # AgentError hierarchy
│   │   ├── progress.py      # VERIFY_OK_LINE constant + emitters
│   │   └── providers/       # LLM provider abstraction
│   │       ├── base.py      # LLMProvider Protocol
│   │       ├── anthropic.py
│   │       ├── openai.py
│   │       ├── deepseek.py
│   │       ├── ollama.py
│   │       └── alibaba.py
│   ├── tests/               # 16 Python test files
│   ├── main.py              # CLI entry point for `python -m ail_agent`
│   └── pyproject.toml
├── examples/
│   └── wallet_service/      # Reference domain for E2E tests
├── Cargo.toml               # Rust workspace config
└── GETTING_STARTED.md       # Setup guide
```

---

## Build & Test

### Rust

```bash
# Build everything
cargo build

# Run all Rust tests
cargo test

# Build release binary
cargo build --release
```

### Python agents

```bash
cd agents

# Install with dev dependencies (pick providers as needed)
pip install -e ".[dev,anthropic]"      # Anthropic only
pip install -e ".[dev,all]"            # All providers

# Run tests (no network, no binary required)
pytest

# Run integration tests (requires ANTHROPIC_API_KEY + built `ail` binary)
pytest -m integration
```

---

## CLI Subcommands (`ail`)

| Command | Description |
|---------|-------------|
| `ail init <name>` | Scaffold a new AIL project directory |
| `ail build` | Run pipeline, emit Python output (`--target typescript` for TS) |
| `ail verify [file]` | Verify constraints via Z3 without emitting |
| `ail context` | Print CIC context packet (`--task TEXT` or `--node NAME`) |
| `ail test` | Build + run generated pytest contract tests |
| `ail run` | Build and run the project entry point |
| `ail serve` | Start the MCP server (stdio transport) |
| `ail status` | Print project summary (node count, graph health) |
| `ail search <query>` | BM25 / semantic search over graph nodes |
| `ail reindex` | Rebuild BM25 + embedding indexes |
| `ail migrate` | Migrate a graph from one schema version to another |
| `ail export` | Export graph to JSON |
| `ail coverage` | Run semantic coverage analysis (SCFT) |
| `ail agent <task>` | Launch the Python LangGraph agent for a task |

---

## MCP Server Tools (10 total)

The MCP server (`ail serve`) exposes JSON-RPC tools over stdio transport.

**Read tools (5):** `ail.status`, `ail.structure`, `ail.context`, `ail.search`, `ail.review`

**Write tools (5):** `ail.write`, `ail.batch`, `ail.build`, `ail.verify`, `ail.delete` / `ail.move` (via patch)

The Python agent connects via `MCPToolkit` (sync wrapper, 30 s call timeout, 5 s connect timeout).

---

## Agent Layer

### Workflow (LangGraph)

```
START → plan → code → verify → done
                ↑_____|        ↓
                            error
```

`AILAgentState` (TypedDict, JSON-serializable):
- `status`: `plan | code | verify | done | error`
- `task`, `plan`, `current_step`, `iteration`
- `node_id_map`: label → UUID resolved incrementally by coder
- `model`, `mcp_port`, `max_iterations` (default 50), `steps_per_plan` (default 20)

### Launching the agent

```bash
# Via Rust CLI (recommended)
ail agent "add transfer_money node with balance guard" --model anthropic:claude-sonnet-4-5

# Via Python directly
python -m ail_agent "add transfer_money node" --model anthropic:claude-sonnet-4-5 --mcp-port 7777
```

### LLM Providers

| Provider | Install extra | Model spec prefix | Env var |
|----------|--------------|-------------------|---------|
| Anthropic | `pip install -e ".[anthropic]"` | `anthropic:` | `ANTHROPIC_API_KEY` |
| OpenAI | `pip install -e ".[openai]"` | `openai:` | `OPENAI_API_KEY` |
| DeepSeek | `pip install -e ".[deepseek]"` | `deepseek:` | `DEEPSEEK_API_KEY` |
| Ollama | `pip install -e ".[ollama]"` | `ollama:` | — (local) |
| Alibaba/Qwen | `pip install -e ".[alibaba]"` | `alibaba:` | `DASHSCOPE_API_KEY` |

Provider SDKs are lazy-imported — `ImportError` surfaces only when the provider is actually used, not at module load.

---

## Key Architectural Invariants

1. **`AILAgentState` must stay JSON-serializable** — no Python objects as values, only scalars and JSON-safe containers.
2. **Workers never raise** — `run_planner`, `run_coder`, `run_verify` catch all `AgentError` subclasses and return `status="error"` state.
3. **`coder` applies one step per invocation** — LangGraph calls it in a loop; each call advances `current_step` by 1.
4. **MCP server is single-threaded** — `McpServer` uses `RefCell`/`Cell`; never share across threads.
5. **`dirty` flag** — set by any write tool, cleared on successful pipeline refresh; controls whether verify/build re-parses from disk or in-memory graph.
6. **Provider abstraction is a Protocol** — any class with `complete()` and `complete_with_tools()` is a valid provider; no base class required.

---

## Testing Conventions

- **`FakeProvider`** — test double for `LLMProvider`, accepts a string (return value) or `Exception` (to raise).
- **`FakeToolkit`** — test double for `MCPToolkit`, records calls, returns preset responses.
- **`@pytest.mark.integration`** — marks tests that require live API keys + built `ail` binary; excluded from default `pytest` run.
- **Wallet service** (`examples/wallet_service/`) — canonical E2E domain; used in `test_workflow_e2e_wallet.py`.

---

## Environment Setup Summary

```bash
# 1. Build Rust toolchain
cargo build

# 2. Install Python agent layer
cd agents && pip install -e ".[dev,anthropic]"

# 3. Set API key
export ANTHROPIC_API_KEY=sk-ant-...

# 4. Start MCP server in one terminal
ail serve

# 5. Run agent in another terminal
ail agent "your task here" --model anthropic:claude-sonnet-4-5
```
