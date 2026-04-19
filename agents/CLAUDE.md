# CLAUDE.md — ail-agent (Python Layer)

## Overview

`ail-agent` is the Python LangGraph agent layer for AIL v3.0. It orchestrates an AI coding loop that uses the Rust MCP server (`ail serve`) as its tool backend.

**Python ≥ 3.10 required.**

---

## Package Structure

```
agents/
├── ail_agent/
│   ├── orchestrator.py   # LangGraph state machine + AILAgentState TypedDict
│   ├── planner.py        # LLM → structured plan (run_planner worker)
│   ├── coder.py          # Plan step → MCP ail.write (run_coder worker)
│   ├── verify.py         # Post-code ail.status check (run_verify worker)
│   ├── mcp_toolkit.py    # Sync MCP client (daemon thread + asyncio loop)
│   ├── prompts.py        # PLANNER_SYSTEM_PROMPT + PLANNER_USER_TEMPLATE
│   ├── plan_format.py    # PlanStep TypedDict + parse_plan()
│   ├── errors.py         # AgentError hierarchy
│   ├── progress.py       # VERIFY_OK_LINE constant
│   ├── registry.py       # Provider registry: name → LLMProvider factory
│   └── providers/
│       ├── base.py       # LLMProvider Protocol
│       ├── anthropic.py  # Anthropic SDK wrapper
│       ├── openai.py     # OpenAI SDK wrapper
│       ├── deepseek.py   # DeepSeek via OpenAI-compat
│       ├── ollama.py     # Ollama via httpx
│       └── alibaba.py    # Alibaba/Qwen via OpenAI-compat
├── tests/                # 16 test files — all offline by default
├── main.py               # __main__ entry point
└── pyproject.toml
```

---

## Installation

```bash
cd agents

# Minimal dev install (Anthropic provider)
pip install -e ".[dev,anthropic]"

# All providers
pip install -e ".[dev,all]"

# Provider extras: anthropic | openai | deepseek | alibaba | ollama
```

---

## Running

```bash
# Via Rust CLI (recommended — handles MCP server lifecycle)
ail agent "add transfer_money node" --model anthropic:claude-sonnet-4-5

# Via Python directly (you must start `ail serve` separately first)
python -m ail_agent "add transfer_money node" --model anthropic:claude-sonnet-4-5
```

**Flags:**
- `--model <provider:model-id>` — e.g. `anthropic:claude-sonnet-4-5`, `openai:gpt-4o`
- `--mcp-port <port>` — default 7777 (reserved; current transport is stdio)
- `--max-iterations <n>` — default 50
- `--steps-per-plan <n>` — default 20

---

## Agent State Machine

```
START → plan → code (loop, one step/call) → verify → done
                ↑___________________________|         ↓
                                                   error
```

`AILAgentState` fields (all JSON-serializable):

| Field | Type | Description |
|-------|------|-------------|
| `status` | str | `plan\|code\|verify\|done\|error` |
| `task` | str | Natural-language task from the user |
| `plan` | list[dict] \| None | PlanStep list from planner |
| `current_step` | int | Index into plan (advances by 1 per coder call) |
| `iteration` | int | Total LangGraph iterations (bounded by max_iterations) |
| `node_id_map` | dict | label → UUID, resolved incrementally |
| `error` | str \| None | Error message when status="error" |
| `model` | str \| None | Provider model spec |
| `mcp_port` | int | MCP port (default 7777) |
| `max_iterations` | int | Hard iteration cap (default 50) |
| `steps_per_plan` | int | Per-plan step budget (default 20) |

---

## LLM Providers

| Key | SDK | Env var |
|-----|-----|---------|
| `anthropic` | `anthropic` | `ANTHROPIC_API_KEY` |
| `openai` | `openai` | `OPENAI_API_KEY` |
| `deepseek` | `openai` (compat) | `DEEPSEEK_API_KEY` |
| `ollama` | `httpx` | — (local) |
| `alibaba` | `openai` (compat) | `DASHSCOPE_API_KEY` |

`LLMProvider` is a **Protocol** — implement `complete(system, user, model)` and `complete_with_tools(...)` to add a new provider.

Provider SDKs are lazy-imported; `ImportError` only raises at first use.

---

## MCPToolkit

`MCPToolkit` is a synchronous context manager wrapping the async MCP SDK:

```python
with MCPToolkit(server_command="ail", server_args=["serve"]) as tk:
    result = tk.call("ail.status", {})
    result = tk.call("ail.write", {"pattern": "Do", "intent": "transfer money"})
```

- Spawns a daemon thread with its own asyncio event loop.
- Connects to `ail serve` via stdio transport (5 s connect timeout).
- Each `call()` blocks the calling thread via `asyncio.run_coroutine_threadsafe` (30 s call timeout).
- `port` parameter is reserved; ignored in current implementation.

---

## Tests

```bash
cd agents

# All offline tests (no API keys, no binary needed)
pytest

# With verbose output
pytest -v

# Single file
pytest tests/test_orchestrator.py

# Integration tests (requires ANTHROPIC_API_KEY + built `ail` binary)
pytest -m integration
```

**Test doubles:**
- `FakeProvider` — accepts a string (return value) or `Exception` (to raise); records all calls.
- `FakeToolkit` — records MCP calls, returns preset response dicts.

**Reference domain:** `examples/wallet_service/` — used in `test_workflow_e2e_wallet.py`.

---

## Key Invariants

1. **Workers never raise** — `run_planner`, `run_coder`, `run_verify` catch all `AgentError` subclasses and return `status="error"` state instead.
2. **State is JSON-serializable** — never put Python objects into `AILAgentState` values.
3. **Coder applies one step per call** — LangGraph loops back; each invocation increments `current_step` by exactly 1.
4. **`node_id_map` is additive** — labels are resolved to UUIDs once and cached; never removed mid-plan.
