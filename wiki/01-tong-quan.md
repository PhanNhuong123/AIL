# Tổng quan repo AIL

## AIL là gì?

AIL, viết tắt từ **AI Layer**, là một lớp trung gian giữa AI coding assistant và code ứng dụng. Người dùng mô tả hệ thống bằng file `.ail` dạng ngôn ngữ tự nhiên có cấu trúc; AIL parse mô tả đó thành graph, áp dụng ràng buộc, kiểm chứng bằng Z3, rồi sinh code chạy được.

Ý tưởng trung tâm của repo là: constraint không nên chỉ nằm trong prompt, comment hoặc tài liệu rời. Constraint cần được biểu diễn trong graph của hệ thống, có thể kế thừa, kiểm chứng và truy vết đến code sinh ra.

## Vấn đề chính

Khi AI viết code qua nhiều phiên làm việc, nó dễ "quên" constraint đã từng được nói trước đó, ví dụ "balance không được âm". AIL cố gắng biến các constraint như vậy thành một phần của mô hình hệ thống:

- constraint được khai báo ở type, function hoặc node;
- constraint lan truyền qua quan hệ cha/con, bước trước/sau và type usage;
- verifier phát hiện vi phạm trước khi code được emit;
- source map giúp liên kết code sinh ra với node `.ail` gốc.

## Pipeline lớn

Pipeline được mô tả trong README và code theo hướng:

```text
.ail text
  -> parse thành AilGraph
  -> validate graph
  -> type-check / contract-check
  -> Z3 verification
  -> emit Python / TypeScript
  -> test, source map, runtime guard
```

Trong README, pipeline còn được diễn đạt bằng các trạng thái kiểu:

```text
AilGraph -> ValidGraph -> TypedGraph -> VerifiedGraph -> emitted code
```

Điểm quan trọng là emitter nhận graph đã verify, nhằm giảm khả năng sinh code từ spec chưa hợp lệ.

## CIC: Constraint Inheritance Chain

CIC là cơ chế lan truyền constraint qua graph. README mô tả bốn hướng chính:

| Hướng | Ý nghĩa |
|---|---|
| DOWN | Constraint từ node cha đi xuống các node con. |
| UP | Sự thật đã verify ở node con có thể được đưa lên ngữ cảnh cha. |
| ACROSS | Output/fact của bước trước đi vào bước sau. |
| DIAGONAL | Constraint gắn với type được inject vào mọi nơi dùng type đó. |

Từ góc nhìn người dùng, một constraint viết một lần ở type hoặc parent node có thể ảnh hưởng đến nhiều function/step bên dưới.

## Các mảng chính trong repo

| Khu vực | Vai trò |
|---|---|
| `crates/ail-graph` | Graph core, node/edge/model, validation, CIC context, BM25 search nền. |
| `crates/ail-types` | Type system, typed graph, expression/constraint typing. |
| `crates/ail-contract` | Static contract checks, Z3 encoding/verification, sheaf consistency. |
| `crates/ail-text` | Parser/renderer cho cú pháp `.ail`. |
| `crates/ail-emit` | Emit Python, TypeScript, scaffold, test stubs, source map. |
| `crates/ail-db` | SQLite backend, persistence, invalidation/cache support. |
| `crates/ail-search` | Search provider, BM25/hybrid/embedding infrastructure. |
| `crates/ail-coverage` | Semantic coverage: đo mức child node bao phủ intent của parent. |
| `crates/ail-mcp` | MCP server và tool I/O để AI agent đọc/ghi graph. |
| `crates/ail-cli` | CLI gom các workflow init/build/verify/test/search/context/agent. |
| `crates/ail-runtime-py` | Runtime helper Python cho generated code. |
| `agents` | Python AI agent orchestration, providers, planner/coder/verifier loop. |
| `ide` | SvelteKit + Tauri desktop IDE, canvas/stage UI, chat, watcher và agent event integration. |
| `examples/wallet_service` | Ví dụ end-to-end quan trọng nhất để đọc và test hành vi. |

## Người dùng tương tác như thế nào?

Luồng cơ bản trong `GETTING_STARTED.md`:

```bash
ail init hello_wallet
cd hello_wallet
ail build
ail verify
ail test
```

Luồng nâng cao hơn:

```bash
ail migrate --from src/ --to project.ail.db --verify
ail search "balance transfer"
ail context --node transfer_money
ail serve
ail agent "add error handling to transfer_money"
```

## Tình trạng sản phẩm

Theo README, repo đang ở trạng thái phát triển chủ động. Các phần v1/v2/v3 đã có trong code và docs: graph/CIC, type system, Z3 verification, parser, Python/TypeScript emitters, SQLite backend, embedding search, MCP write tools, semantic coverage và AI Agent Foundation. Roadmap tiếp theo hướng tới IDE/Tauri, agent trên canvas, sheaf consistency, debug/runtime tracing và ecosystem.

## Tài liệu hiện có nên đọc kèm

- `README.md`: mô tả sản phẩm, status, roadmap, license.
- `GETTING_STARTED.md`: hướng dẫn dùng CLI từ đầu.
- `docs/config-reference.md`: trạng thái các field trong `ail.config.toml`.
- `docs/README.md`: bản đồ tài liệu versioned plan/status/roadmap.
- `agents/README.md`: cách cài và chạy Python agent.
- `ide/README.md`: cách chạy desktop IDE và ghi chú Tauri workspace.
- `examples/wallet_service/README.md`: ví dụ cụ thể để thử nghiệm.
