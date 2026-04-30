# Testing, cấu hình và vận hành

Tài liệu này gom các bề mặt kiểm thử, cấu hình và vận hành hiện có trong AIL. Nguồn chính là `README.md`, `GETTING_STARTED.md`, `CHANGELOG.md`, `MIGRATION.md`, `docs/config-reference.md`, `agents/README.md`, `examples/wallet_service/`, các test trong `crates/*/tests` và `agents/tests`.

## Bức tranh nhanh

AIL có hai lớp kiểm thử chính:

| Lớp | Lệnh thường dùng | Phạm vi |
|---|---|---|
| Rust workspace | `cargo test --workspace` | Parser, graph/CIC, type checker, contract verifier, emitter, SQLite, search, coverage, MCP, CLI, UI bridge. |
| Python agent/runtime | `python -m pytest agents/tests/` và test runtime Python | Planner/coder/verifier loop, provider adapters, MCP toolkit, retry, CLI Python, runtime contract helpers. |
| Generated project | `ail test`, `pytest generated/test_contracts.py`, `npx vitest run` | Test code sinh ra từ `.ail` sau `ail build`. |

Ví dụ end-to-end quan trọng nhất là `examples/wallet_service`. Các test CLI thường copy ví dụ này vào tempdir để migration/build/search/context không làm bẩn fixture gốc.

## Chạy test Rust

Từ root repo:

```bash
cargo test --workspace
```

Chạy một crate hoặc một test file:

```bash
cargo test -p ail-cli
cargo test -p ail-cli --test cli_e2e_wallet_sqlite
cargo test -p ail-text --test parse_patterns
```

Các nhóm test đáng chú ý:

| Khu vực | Test tiêu biểu | Ý nghĩa |
|---|---|---|
| CLI | `crates/ail-cli/tests/*` | `init`, `build`, `verify`, `test`, `migrate`, `export`, `search`, `context`, `coverage`, `agent`, `sheaf`. |
| SQLite | `crates/ail-db/tests/*`, `cli_migrate.rs`, `cli_e2e_wallet_sqlite.rs` | Lưu graph, giữ node/edge/contract/order, export, cache CIC và invalidation coverage. |
| Coverage | `crates/ail-coverage/tests/*`, `coverage_cmd.rs`, `cli_e2e_wallet_coverage.rs` | Semantic coverage, cache, trạng thái Leaf/Full/Partial/Weak/N/A. |
| Search | `crates/ail-search/tests/*` | Cấu hình provider, embedding index, hybrid ranking/RRF. |
| MCP | `crates/ail-mcp/tests/*` | Protocol, read/write tools, `ail.review`, agent workflow qua MCP. |
| Emitter | `crates/ail-emit/tests/*`, `cli_e2e_wallet_ts.rs` | Python/TypeScript output, runtime guards, scaffold/test stubs. |
| UI bridge | `crates/ail-ui-bridge/tests/*` | JSON bridge, watcher, fine-grained patches, lens metrics, verifier/reviewer surfaces cho IDE v4. |

Một số test shell-out có điều kiện:

- TypeScript E2E cần `node`, `npm`, `npx`; đặt `AIL_SKIP_TS_NODE=1` để bỏ qua phần Node shell-out.
- Generated Python test trong CLI sẽ bỏ qua nếu `python -m pytest --version` không chạy được.
- Một số test fake Python trong `cli_agent.rs` chỉ chạy trên non-Windows vì cách Windows ưu tiên `python.exe` qua `PATHEXT`.

## Feature flags và test bị ignore

Các feature quan trọng:

```bash
cargo test -p ail-cli --features embeddings
cargo test -p ail-mcp --features embeddings
cargo test -p ail-cli --features z3-verify
```

`embeddings` bật ONNX/tokenizer cho semantic search và coverage. Các test coverage full path thường bị `#[ignore]` vì cần build với feature này và cần model ở:

```text
~/.ail/models/all-MiniLM-L6-v2/
```

Ví dụ chạy các test ignore có embeddings:

```bash
cargo test -p ail-cli --features embeddings --test cli_e2e_wallet_coverage -- --ignored
cargo test -p ail-cli --features embeddings --test coverage_cmd -- --ignored
cargo test -p ail-mcp --features embeddings --test review_tool_e2e_wallet -- --ignored
```

`z3-verify` không phải default feature. `CHANGELOG.md` ghi nhận với feature này có một số test cũ trong `crates/ail-mcp/tests/ai_workflow.rs` có thể fail trên fixture `wallet_full` vì fixture thiếu constraint `amount <= sender.balance`; default build không bị ảnh hưởng.

## Chạy test Python

Agent Python:

```bash
pip install -e "./agents/[dev]"
python -m pytest agents/tests/
```

Các file test chính trong `agents/tests`:

| File | Nội dung |
|---|---|
| `test_orchestrator.py`, `test_workflow_e2e_mocked.py`, `test_workflow_e2e_wallet.py` | LangGraph state machine, plan/code/verify loop, wallet flow mocked. |
| `test_planner.py`, `test_plan_format.py`, `test_coder.py`, `test_verify.py` | Parser plan JSON, coder budget guard, verify worker. |
| `test_providers.py`, `test_provider_swap.py`, `test_registry.py`, `test_retry.py` | Provider adapters, `provider:model`, API key errors, retry. |
| `test_mcp_toolkit.py`, `test_cli_main.py`, `test_progress_json.py`, `test_errors.py` | MCP facade, CLI exit code, progress JSON, error hierarchy. |
| `test_integration_live.py` | Live integration, skip mặc định. |

Live agent integration bị skip mặc định. Muốn chạy cần `ail` trên `PATH`, API key Anthropic và env bật test:

```bash
set AIL_RUN_LIVE_INTEGRATION=1
set ANTHROPIC_API_KEY=sk-...
python -m pytest agents/tests/test_integration_live.py -m integration
```

Trên shell Unix:

```bash
AIL_RUN_LIVE_INTEGRATION=1 ANTHROPIC_API_KEY=sk-... python -m pytest agents/tests/test_integration_live.py -m integration
```

Runtime Python helper ở `crates/ail-runtime-py` có test riêng cho `pre`, `post`, `keep` và `ContractViolation`:

```bash
pip install -e crates/ail-runtime-py
python -m pytest crates/ail-runtime-py/tests/
```

Lưu ý nhỏ: `agents/README.md` yêu cầu Python 3.11+, còn `agents/pyproject.toml` khai báo `requires-python = ">=3.10"`. Khi setup mới, ưu tiên 3.11+ theo README vì đó là đường vận hành được tài liệu hóa.

## Test generated output

Luồng Python generated:

```bash
cd examples/wallet_service
ail build --target python
pytest generated/test_contracts.py -v
```

Hoặc dùng CLI wrapper:

```bash
ail test
```

`ail test` build trước, rồi chạy pytest trên test contracts sinh ra. Theo `GETTING_STARTED.md`, generated stubs có thể dùng `pytest.skip()` để ghi lại hình dạng contract; phần code developer-owned nằm trong `scaffolded/` không bị overwrite sau lần đầu.

Luồng TypeScript generated:

```bash
cd examples/wallet_service
ail build --target typescript
cd dist-ts
npm install
npx tsc --noEmit
npx vitest run
```

`cli_e2e_wallet_ts.rs` kiểm tra layout `dist-ts/`, strict `tsconfig`, runtime `pre/post/keep`, parity với Python emitter, và runtime factory ném lỗi khi input vi phạm constraint.

## Cấu hình `ail.config.toml`

Theo `docs/config-reference.md`, v3.0.0 chỉ có ba nhóm config được CLI đọc thật sự:

| Section | Trạng thái | Ghi chú |
|---|---|---|
| `[database] backend` | ACTIVE | `auto`, `sqlite`, `filesystem`. |
| `[coverage]` | ACTIVE | Dùng bởi `ail coverage`; cần `embeddings` để tính điểm. |
| `[agent]` | ACTIVE | Dùng bởi `ail agent`; CLI flag override TOML. |

Các section như `[project]`, `[build]`, `[build.typescript]`, `[search]` hiện là schema/documentation placeholder. Build target, contracts và source map vẫn được điều khiển bằng CLI flag như `ail build --target typescript`, `--contracts`, `--source-map`.

Ví dụ config tối thiểu của wallet service:

```toml
[project]
name = "wallet_service"
version = "0.2.0"

[build]
target = "python"
contracts = "on"
source_map = false

[database]
backend = "auto"
```

Ví dụ config đầy đủ hơn cho vận hành:

```toml
[database]
backend = "auto"

[coverage]
enabled = true
threshold_full = 0.8
threshold_partial = 0.5
extra_concepts = ["error handling", "observability"]

[agent]
model = "openai:gpt-4o"
max_iterations = 100
steps_per_plan = 30
```

Unsupported key bị bỏ qua im lặng. Ví dụ `timeout_seconds` trong `[agent]` chưa được Python side consume ở v3.0.

## Database, migration và rollback

Filesystem `.ail` vẫn là default khi không có `project.ail.db`. SQLite là opt-in từ v2.0:

```bash
cd examples/wallet_service
ail migrate --from src/ --to project.ail.db --verify
```

Backend resolution:

1. `--from-db <path>` thắng mọi config và ép SQLite.
2. `[database] backend = "sqlite"` ép dùng `project.ail.db`, lỗi nếu file thiếu.
3. `[database] backend = "filesystem"` ép filesystem.
4. `auto` dùng SQLite nếu `project.ail.db` nằm cạnh `ail.config.toml`, nếu không thì filesystem.

Sau migration:

```bash
ail verify
ail build --target python
ail build --target typescript
ail search "transfer money"
ail context --task "add rate limiting"
```

Inspect database bằng export:

```bash
ail export --from project.ail.db --to exported/
```

Output hiện là một file `exported/export.ail`, không phải cây per-node. Rollback đơn giản: xóa `project.ail.db` và các file SQLite phụ `project.ail.db-wal`, `project.ail.db-shm`; cây `.ail` filesystem không bị `ail migrate` sửa.

Nên ignore các file WAL/SHM:

```gitignore
project.ail.db-wal
project.ail.db-shm
```

## Coverage, review và cache

Semantic coverage trả lời câu hỏi: child node đã bao phủ intent của parent đến mức nào. Lệnh chính:

```bash
ail coverage --node transfer_money
ail coverage --all
ail coverage --warm-cache
ail coverage --from-db project.ail.db --all
```

Điều kiện vận hành:

- Cần SQLite backend; filesystem-only sẽ báo lỗi có nhắc SQLite.
- Cần build có feature `embeddings` để tính semantic score bằng ONNX.
- Nếu `[coverage] enabled = false`, lệnh trả notice và bỏ qua scoring.
- Cache coverage nằm trong SQLite; khi child node bị update qua DB/MCP write path, ancestor coverage bị invalidated.

Trạng thái coverage gồm `Full`, `Partial`, `Weak`, `N/A`, `Leaf`, `Unavailable`. `ail.review` trên MCP trả coverage và missing-aspect data cho một node, và vẫn có schema hợp lệ khi embeddings không sẵn sàng.

## Search và context

BM25 search chạy trên SQLite/FTS5, không cần ONNX:

```bash
ail search "balance transfer"
ail search "transfer" --budget 5
ail search "transfer money" --bm25-only
```

Hybrid semantic search cần feature `embeddings` và model ONNX:

```bash
cargo build -p ail-cli --features embeddings --release
ail search --setup
ail reindex --embeddings
ail search "balance transfer" --semantic
```

Nếu build không có `embeddings`, một số path `--semantic` fallback về BM25 hoặc báo diagnostic tùy command. OpenAI search provider hiện chỉ là shape trong enum/config; provider chạy được hôm nay là local ONNX.

Context packet:

```bash
ail context --task "validate transfer"
ail context --node transfer_money
```

`ail context` cần SQLite. Lần đầu tạo row trong `cic_cache`, lần sau cùng target có thể hit cache. Packet có inherited constraints và `promoted_facts` từ path-sensitive CIC.

## Agent operations

Cài agent:

```bash
pip install ./agents/
pip install -e "./agents/[all,dev]"
```

Chạy:

```bash
cd examples/wallet_service
ail agent "add error handling to transfer_money"
```

Provider được chọn bằng `provider:model`:

| Provider | Ví dụ | Env var |
|---|---|---|
| Anthropic | `anthropic:claude-sonnet-4-5` | `ANTHROPIC_API_KEY` |
| OpenAI | `openai:gpt-4o` | `OPENAI_API_KEY` |
| DeepSeek | `deepseek:deepseek-chat` | `DEEPSEEK_API_KEY` |
| Alibaba/Qwen | `qwen:qwen-max` | `DASHSCOPE_API_KEY` |
| Ollama | `ollama:llama3.1` | `OLLAMA_BASE_URL` optional |

CLI flags:

```bash
ail agent --model openai:gpt-4o --max-iterations 100 --steps-per-plan 30 "task"
```

Thứ tự precedence là CLI flag -> `[agent]` TOML -> default Python. `--mcp-port` hiện reserved cho network MCP tương lai; agent dùng stdio và spawn `ail serve`.

Exit code theo `agents/README.md`:

| Code | Ý nghĩa |
|---|---|
| 0 | Workflow đạt `done`. |
| 1 | Workflow đạt `error`. |
| 2 | Gọi sai CLI hoặc model spec lỗi. |
| 3 | MCP unavailable / `AIL-G0145`. |
| 130 | Ctrl-C. |

## Troubleshooting nhanh

| Triệu chứng | Cách xử lý |
|---|---|
| `ail context` hoặc `ail coverage` báo cần SQLite | Chạy `ail migrate --from src/ --to project.ail.db --verify` hoặc dùng `--from-db`. |
| `ail coverage` không tính điểm | Kiểm tra build có `--features embeddings`, model ONNX tồn tại, `[coverage] enabled` không phải `false`. |
| `ail search --semantic` không ra hybrid | Chạy `ail search --setup`, đặt model ở `~/.ail/models/all-MiniLM-L6-v2/`, build CLI với `embeddings`, rồi `ail reindex --embeddings`. |
| Agent báo missing env var | Export đúng API key: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `DEEPSEEK_API_KEY`, `DASHSCOPE_API_KEY`. |
| Agent báo unknown provider prefix | Dùng một trong `anthropic`, `openai`, `deepseek`, `alibaba`, `qwen`, `ollama` với dạng `<prefix>:<model>`. |
| `MCPConnectionError` / `AIL-G0145` | Kiểm tra `ail --version`, `ail serve`, `ail.config.toml`, và current directory là project AIL. |
| `StepBudgetError` / `AIL-G0143` | Tăng `--steps-per-plan` hoặc chia nhỏ task. |
| Ollama bị treo | Đảm bảo daemon chạy ở `OLLAMA_BASE_URL` hoặc default `http://localhost:11434`; v3.0 chưa có preflight health check. |
| TypeScript E2E fail do toolchain | Kiểm tra `node`, `npm`, `npx`; có thể đặt `AIL_SKIP_TS_NODE=1` khi chỉ muốn chạy Rust-only assertions. |
| Windows build liên quan Z3/LLVM | `GETTING_STARTED.md` ghi chú cần set `LIBCLANG_PATH` tới LLVM `bin` trước khi build. |

## Status, release và roadmap

Repo đang ở trạng thái active development. README badge nói `v3.0.0 - coverage + agent`, còn phần Status mô tả "v0.1 in active development" như nhãn sản phẩm sớm.

Theo `CHANGELOG.md`, release v3.0.0 ngày 2026-04-20 thêm:

- `ail coverage` và semantic coverage cache/invalidation;
- Python LangGraph agent foundation với 5 provider;
- `ail agent` trong CLI;
- `[coverage]` và `[agent]` TOML active;
- `ail.review` MCP tool.

Release procedure v3.0.0 được ghi là:

```bash
cargo test --workspace
python -m pytest agents/tests/
cargo build --release
python -m venv .venv-v3
.venv-v3/Scripts/pip install ./agents/
.venv-v3/Scripts/ail-agent --help
```

Roadmap hiện trỏ sang v4.0. Scope v4.0 là Tauri/Svelte visual IDE với shell `TitleBar | Outline | Stage | Chat`, bridge JSON/patch/watcher, chat-to-agent streaming, verifier/reviewer lens, sidecar packaging và sheaf consistency. Kế tiếp v5+ dự kiến runtime tracing, interactive debug, entropy analysis, multi-agent workflows và phân phối production.

## License

Core AIL dùng Business Source License 1.1 (`BUSL-1.1`). Non-production use được phép; personal, academic, research, educational, evaluation và non-commercial open-source production use cũng được grant thêm. Commercial production hoặc embedding core vào sản phẩm thương mại cần commercial license riêng.

Runtime packages và generated runtime helpers được MIT ngay cho ứng dụng sinh ra. Core BUSL đổi sang MIT vào ngày 2030-04-19 hoặc kỷ niệm bốn năm từ lần phân phối public đầu tiên của version đó, tùy mốc nào đến trước. Code sinh ra từ input của người dùng bằng tool AIL không thuộc Licensed Work.
