# AIL Agent

`ail_agent` is the Python side of the AIL v3.0 agent foundation. It is a
LangGraph workflow that plans a task, writes nodes into an AIL graph via the
MCP write tools, and re-verifies through the hard Rust pipeline. The
`ail agent "<task>"` CLI in `ail-cli` is a thin subprocess wrapper around
this package.

## Requirements

- Python 3.11 or newer.
- The `ail` binary on `PATH` — the agent spawns `ail serve` over stdio for
  MCP access.
- Exactly one provider API key (see the Providers table below). Ollama is
  the only provider that does not require a key.

## Install

From the repository root:

```bash
pip install ./agents/              # production install
pip install -e ./agents/           # editable install for local development
pip install './agents/[all]'       # all five provider SDKs + httpx
```

Per-provider extras (use these when you only need one SDK):

| Extra           | Pulls in                         |
|-----------------|----------------------------------|
| `[anthropic]`   | `anthropic==0.96.0`              |
| `[openai]`      | `openai==2.32.0`                 |
| `[deepseek]`    | `openai==2.32.0` (OpenAI-compat) |
| `[alibaba]`     | `openai==2.32.0` (OpenAI-compat) |
| `[ollama]`      | `httpx==0.28.1`                  |
| `[all]`         | all of the above                 |
| `[dev]`         | `pytest`, `pytest-asyncio`       |

Example:

```bash
pip install './agents/[openai,dev]'
```

## Quick start

```bash
export ANTHROPIC_API_KEY=sk-ant-...
cd examples/wallet_service
ail agent "add error handling to transfer_money"
```

The agent streams progress to stderr via `progress.emit` and exits 0 on
success.

## Providers

| Provider   | Example model                              | Env var                | Extra          | Notes                               |
|------------|--------------------------------------------|------------------------|----------------|-------------------------------------|
| `anthropic`| `anthropic:claude-sonnet-4-5` (default)    | `ANTHROPIC_API_KEY`    | `[anthropic]`  | Default model.                      |
| `openai`   | `openai:gpt-4o`                            | `OPENAI_API_KEY`       | `[openai]`     | Native tool-calling.                |
| `deepseek` | `deepseek:deepseek-chat`                   | `DEEPSEEK_API_KEY`     | `[deepseek]`   | `deepseek-reasoner` uses prompt fallback. |
| `alibaba` / `qwen` | `qwen:qwen-max`                    | `DASHSCOPE_API_KEY`    | `[alibaba]`    | `qwen-turbo` uses prompt fallback.  |
| `ollama`   | `ollama:llama3.1`                          | `OLLAMA_BASE_URL` (optional) | `[ollama]` | No preflight health check.          |

## Provider swap examples

```bash
# Anthropic (default)
ail agent "refactor transfer_money for clarity"

# OpenAI
ail agent --model openai:gpt-4o "refactor transfer_money for clarity"

# DeepSeek
ail agent --model deepseek:deepseek-chat "refactor transfer_money for clarity"

# Alibaba / Qwen (alias `qwen:`)
ail agent --model qwen:qwen-max "refactor transfer_money for clarity"

# Ollama (local)
ail agent --model ollama:llama3.1 "refactor transfer_money for clarity"
```

## CLI flags

| Flag                | Type    | Default (CLI → TOML → Python)          | Description                                                              |
|---------------------|---------|----------------------------------------|--------------------------------------------------------------------------|
| `--model`           | string  | → `[agent] model` → `anthropic:claude-sonnet-4-5` | Provider:model spec.                                        |
| `--max-iterations`  | integer | → `[agent] max_iterations` → `50`      | Planner/coder/verify loop cap (AIL-G0142).                               |
| `--steps-per-plan`  | integer | → `[agent] steps_per_plan` → `20`      | Coder budget guard per plan (AIL-G0143).                                 |
| `--mcp-port`        | u16     | `7777`                                 | Reserved for a future network-MCP transport; currently ignored (stdio). |

CLI flags override TOML values. When both are absent, the Python side
applies its own defaults.

## Configuration via `ail.config.toml`

The `[agent]` section is **ACTIVE** as of v3.0.0 and is read by
`ail agent` at runtime:

```toml
[agent]
model = "openai:gpt-4o"
max_iterations = 100
steps_per_plan = 30
```

Unknown keys (including `timeout_seconds`, which is reserved for v3.1+)
are silently ignored. Full field status table:
[`docs/config-reference.md#agent`](../docs/config-reference.md).

## Exit codes

| Code | Meaning                                               |
|------|-------------------------------------------------------|
| 0    | Success — workflow reached `done`.                    |
| 1    | Agent error — workflow reached `error` status.        |
| 2    | Bad invocation — CLI parsing failure, unknown model.  |
| 3    | MCP unavailable — `MCPConnectionError` (AIL-G0145).   |
| 130  | SIGINT (Ctrl-C).                                      |

## Troubleshooting

- **`ProviderConfigError: missing environment variable X`** — export the
  API key named in the error. Check the Providers table above.
- **`ProviderConfigError: unknown provider prefix`** — valid prefixes are
  `anthropic`, `openai`, `deepseek`, `alibaba`, `qwen`, `ollama`. The
  format is `<prefix>:<model>`.
- **`MCPConnectionError` (AIL-G0145)** — the agent could not spawn `ail
  serve`. Confirm `ail --version` works from the same shell and that
  `ail.config.toml` exists in the current directory.
- **`StepBudgetError` (AIL-G0143)** — the coder ran out of steps for its
  plan. Raise `--steps-per-plan` or rephrase the task to need fewer
  writes.
- **Ollama hang** — the local daemon must be running and reachable at
  `OLLAMA_BASE_URL` (default `http://localhost:11434`). There is no
  preflight check in v3.0; a missing daemon surfaces only on the first
  request.
- **Slow `deepseek-reasoner` / `qwen-turbo`** — these models use the
  prompt-fallback path rather than native tool-calling. For throughput,
  prefer `deepseek-chat` or `qwen-max`.

## Makefile targets

```bash
make -C agents install-agents       # pip install .
make -C agents install-agents-dev   # pip install -e .[dev]
make -C agents install-agents-all   # pip install -e .[all,dev]
make -C agents test                 # pytest tests/
make -C agents clean                # remove build/, dist/, caches
make -C agents help                 # show all targets
```

## Development

```bash
pip install -e './agents/[dev]'
python -m pytest agents/tests/
```

Package internals are documented in
[`agents/ail_agent/CLAUDE.md`](./ail_agent/CLAUDE.md); package-level
guidance and task ownership live in [`agents/CLAUDE.md`](./CLAUDE.md).

## See also

- Root [README.md](../README.md) — AIL overview, Status, Roadmap.
- [CHANGELOG.md](../CHANGELOG.md) — per-phase change list.
- [MIGRATION.md](../MIGRATION.md) — v2.0 → v3.0 upgrade path.
