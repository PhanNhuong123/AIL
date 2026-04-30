# CLI, MCP và workflow người dùng

Tài liệu này mô tả cách dùng AIL từ CLI, cách CLI chọn backend `.ail`/SQLite, các tool MCP mà AI agent có thể gọi, và ví dụ end-to-end với `examples/wallet_service`. Nguồn chính: `crates/ail-cli/src`, `crates/ail-mcp/src`, `README.md`, `GETTING_STARTED.md`, `docs/config-reference.md` và `examples/wallet_service`.

## Bức tranh nhanh

CLI `ail` là mặt tiền vận hành chính của repo. Người dùng làm việc trong thư mục project có `ail.config.toml`, thường có `src/*.ail`, rồi chạy:

```bash
ail verify
ail build
ail test
ail migrate --from src/ --to project.ail.db --verify
ail search "balance transfer"
ail context --node transfer_money
ail serve
```

Pipeline cốt lõi của các lệnh build/verify/sheaf là:

```text
parse .ail / load .ail.db
  -> validate graph
  -> type-check
  -> contract verify
  -> emit hoặc trả kết quả
```

Lưu ý quan trọng: CLI hiện không có flag global `--project`. Các ví dụ user-facing giả định bạn `cd` vào thư mục project trước khi chạy lệnh. Module `crates/ail-cli/src/commands/project.rs` cũng không phải subcommand `ail project`; nó là helper nội bộ để chọn backend.

## Cấu trúc project `.ail`

`ail init <name>` tạo một project tối thiểu:

```text
<name>/
├── ail.config.toml
├── src/
│   └── main.ail
├── generated/
└── scaffolded/
```

`generated/` là output do AIL sở hữu và có thể bị ghi đè khi build. `scaffolded/` là phần tạo lần đầu rồi để developer sở hữu.

Ví dụ:

```bash
ail init hello_wallet
cd hello_wallet
ail verify
ail build
```

## Cấu hình `ail.config.toml`

Theo `docs/config-reference.md`, chỉ một số section đang được CLI đọc thật ở runtime:

| Section | Trạng thái | Dùng bởi |
|---|---|---|
| `[database] backend` | ACTIVE | `build`, `verify`, `context`, `coverage`, `sheaf` qua backend resolver. |
| `[coverage]` | ACTIVE | `ail coverage`. |
| `[agent]` | ACTIVE | `ail agent`. |
| `[project]`, `[build]`, `[build.typescript]`, `[search]` | schema/documentation | Chủ yếu để người đọc và tooling biết shape; build/search hiện dùng CLI flag hoặc default. |

Ví dụ tối thiểu:

```toml
[project]
name = "wallet_service"
version = "0.2.0"

[build]
target = "python"
contracts = "on"
source_map = false

[database]
backend = "auto"   # auto | sqlite | filesystem
```

Thứ tự chọn backend:

| Ưu tiên | Hành vi |
|---|---|
| `--from-db <path>` | Ép dùng SQLite ở path đó. |
| `[database] backend = "sqlite"` | Dùng `project.ail.db`; lỗi nếu file không tồn tại cạnh config. |
| `[database] backend = "filesystem"` | Dùng source filesystem. Nếu có `src/`, đọc `src/`; nếu không, đọc current root. |
| `auto` hoặc thiếu config | Dùng `project.ail.db` nếu tồn tại, ngược lại dùng filesystem. |

## Các lệnh CLI chính

| Lệnh | Vai trò | Ghi chú nhanh |
|---|---|---|
| `ail init <name>` | Tạo project mới. | Viết `src/main.ail`, `ail.config.toml`, `generated/`, `scaffolded/`. |
| `ail verify [file] [--from-db PATH]` | Chạy pipeline không emit. | `file` hiện chỉ là path hint; verify vẫn chạy toàn project. |
| `ail build` | Chạy pipeline và emit output. | Python mặc định; TypeScript qua `--target typescript`. |
| `ail test` | Build rồi chạy pytest contract test sinh ra. | Cần `python -m pytest`; nếu không có test thì in `No contract tests to run.` |
| `ail run` | Entry point runtime. | Chưa implement; dùng generated code trực tiếp sau `ail build`. |
| `ail status` | In stage cao nhất đạt được và node/edge/do counts. | Nếu có `.ail.db`, in thêm trạng thái embedding index. |
| `ail migrate` | Chuyển `.ail` filesystem sang SQLite. | Target DB phải chưa tồn tại. |
| `ail export` | Xuất SQLite về text. | Viết một file `<to>/export.ail`. |
| `ail search` | Search trên SQLite. | BM25 mặc định; semantic cần feature/model/embedding index. |
| `ail reindex` | Xóa hoặc rebuild embedding vectors. | `--embeddings` cần build với feature `embeddings` và model local. |
| `ail context` | In CIC context packet. | Cần SQLite backend. |
| `ail coverage` | Đo semantic coverage. | Cần SQLite; tính toán thật cần feature `embeddings`. |
| `ail serve` | Start MCP server qua stdio. | Dùng cho Claude/Cursor/agent, không phải HTTP server. |
| `ail agent <task>` | Chạy Python LangGraph agent. | Shell-out `python -m ail_agent ...`. |
| `ail sheaf` | Tính Čech sheaf nerve và obstruction nếu bật `z3-verify`. | Có `--node`, `--format text|json`, `--from-db`. |

Ví dụ build:

```bash
ail build
ail build --watch
ail build --contracts off
ail build --source-map
ail build --target typescript
ail build --from-db project.ail.db
```

`--check-breaking` và `--check-migration` đã có trong parser nhưng trả `NotImplemented`.

## Migration, search và context

SQLite backend gom graph, contracts, FTS5 index và CIC cache vào `project.ail.db`.

```bash
ail migrate --from src/ --to project.ail.db --verify
ail verify
ail build
ail export --from project.ail.db --to exported/
```

Search BM25 chạy được sau khi có `.ail.db`:

```bash
ail search "balance transfer"
ail search "transfer" --budget 5
ail search "transfer" --bm25-only
```

Semantic search cần build CLI với feature `embeddings`, model ONNX ở `~/.ail/models/all-MiniLM-L6-v2/`, rồi reindex:

```bash
ail search --setup
ail reindex --embeddings
ail search "balance transfer" --semantic
```

Context packet cũng cần SQLite:

```bash
ail context --task "validate transfer"
ail context --node transfer_money
ail context --node transfer_money --from-db project.ail.db
```

`--node` ưu tiên hơn `--task`. Lần gọi đầu có thể tạo cache trong `cic_cache`; lần sau cho cùng node có thể hit cache.

## Coverage và sheaf

Coverage trả lời câu hỏi: child nodes đã bao phủ intent của parent tới mức nào. Các mode loại trừ nhau:

```bash
ail coverage --node transfer_money
ail coverage --all
ail coverage --warm-cache
ail coverage --all --from-db project.ail.db
```

`[coverage]` hỗ trợ `enabled`, `threshold_full`, `threshold_partial`, `extra_concepts`.

`ail sheaf` chạy pipeline rồi dựng Čech nerve:

```bash
ail sheaf
ail sheaf --node transfer_money
ail sheaf --format json
ail sheaf --from-db project.ail.db
```

Khi build với feature `z3-verify`, output có thể thêm H1 obstruction diagnostics.

## MCP server

`ail serve` start MCP server trên stdio. MCP client gửi newline-delimited JSON-RPC 2.0 vào stdin và đọc response từ stdout. Config phổ biến:

```json
{
  "mcpServers": {
    "ail": {
      "command": "ail",
      "args": ["serve"]
    }
  }
}
```

Server xử lý `initialize`, `initialized`, `tools/list`, `tools/call`. Khi start, CLI cố verify project để preload `ProjectContext::Verified`; nếu project lỗi, server vẫn chạy với raw empty context và nhắc chạy `ail verify`.

Tool MCP hiện có:

| Tool | Nhóm | Mục đích |
|---|---|---|
| `ail.search` | read | BM25/hybrid search trên graph; trả node id, score, intent, pattern, path, provenance rank. |
| `ail.review` | read | Review semantic coverage cho một node; trả score/status/children/missing/suggestion. |
| `ail.context` | read | Trả CIC context packet cho task, có primary/secondary/constraints/promoted facts. |
| `ail.verify` | pipeline | Re-run verify toàn project hoặc graph in-memory nếu dirty. |
| `ail.build` | pipeline | Emit Python file metadata từ verified graph; không ghi file trực tiếp như CLI build. |
| `ail.status` | read | Stage hiện tại, node/edge/do counts, root id nếu có. |
| `ail.write` | write | Tạo node mới dưới parent, có pattern/intent/expression/contracts/metadata/position. |
| `ail.patch` | write | Sửa fields của node: intent, expression, pattern, contracts, metadata. |
| `ail.move` | write | Chuyển parent hoặc vị trí sibling của node. |
| `ail.delete` | write | Xóa node theo `cascade`, `orphan` hoặc `dry_run`. |
| `ail.batch` | write | Chạy nhiều write/patch/move/delete theo thứ tự, rollback in-memory nếu lỗi. |

Các write tool mutate graph trong bộ nhớ của server, demote context về `Raw`, set dirty flag và clear search/embedding cache. Sau đó `ail.verify` hoặc `ail.build` trong MCP sẽ pipeline lại từ graph in-memory để không làm mất edit. Những edit này sống trong session MCP; tài liệu code nhắc `SqliteGraph::save_from_graph()` là hướng persist explicit, nhưng `ail serve` hiện không tự ghi ngược ra `.ail`/`.ail.db`.

Ví dụ tool call dạng JSON-RPC rút gọn:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"ail.search","arguments":{"query":"transfer money","budget":5}}}
```

Ví dụ `ail.write`:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "ail.write",
    "arguments": {
      "parent_id": "<uuid>",
      "pattern": "do",
      "intent": "validate sender balance",
      "position": 0,
      "contracts": [
        {"kind": "before", "expression": "sender_balance >= amount"}
      ]
    }
  }
}
```

## Agent workflow

`ail agent <task>` là wrapper CLI cho Python agent:

```bash
pip install -e ./agents
cd examples/wallet_service
ail agent "add error handling to transfer_money"
```

CLI probe `python`, sau đó `python3`, rồi chạy:

```text
python -m ail_agent <task> ...
```

Flags:

```bash
ail agent "add validation" --model openai:gpt-4o
ail agent "add validation" --max-iterations 100 --steps-per-plan 30
```

`--mcp-port` có default `7777` nhưng hiện được giữ cho transport mạng tương lai; implementation hiện dùng `ail serve` qua stdio. CLI flags override `[agent]` trong TOML, rồi mới tới default phía Python.

## Ví dụ `wallet_service`

`examples/wallet_service` là ví dụ end-to-end canonical. Layout chính:

```text
examples/wallet_service/
├── ail.config.toml
├── src/
│   ├── wallet_balance.ail
│   ├── positive_amount.ail
│   ├── user.ail
│   ├── transfer_result.ail
│   ├── add_money.ail
│   ├── deduct_money.ail
│   └── transfer_money.ail
└── project.ail.db   # sinh ra sau migrate
```

`transfer_money.ail` mô tả hàm trừ tiền từ balance với pre/post-condition:

```text
do transfer money
  from sender_balance:WalletBalance, amount:PositiveAmount
  -> WalletBalance

  promise before: sender_balance >= amount
  promise before: amount > 0
  promise after: sender_balance >= 0

  let new_balance:WalletBalance = sender_balance - amount
```

Luồng thử nhanh:

```bash
cd examples/wallet_service
ail migrate --from src/ --to project.ail.db --verify
ail verify
ail build --target python
ail build --target typescript
ail search "balance transfer"
ail context --task "add rate limiting"
ail context --task "add rate limiting"
ail sheaf --node transfer_money
```

Vì `[database] backend = "auto"`, sau khi `project.ail.db` tồn tại thì `verify`, `build`, `context`, `coverage`, `sheaf` có thể tự chọn SQLite, trừ khi bạn ép `--from-db` hoặc chuyển backend về `filesystem`.

