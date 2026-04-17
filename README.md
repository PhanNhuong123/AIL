# AIL — AI Language

**Ngôn ngữ lập trình đầu tiên được thiết kế để AI viết code không thể sai.**

```
Ngôn ngữ truyền thống:  AI viết text code → hy vọng đúng → chạy → crash → sửa → lặp lại
AIL:                     AI viết English có cấu trúc → hệ thống verify → đảm bảo đúng → build ra Python/TS/Rust
```

---

## Vấn Đề

AI (ChatGPT, Claude, Copilot) viết code ngày càng tốt. Nhưng có 1 vấn đề không ai giải được:

**AI quên.**

```
Phút 1:   User nói "balance không được âm"
Phút 5:   AI viết check balance >= amount ✓
Phút 30:  AI thêm feature mới, quên constraint, tạo balance = -20 ✗
```

AI không ngu. AI bị **thiếu thông tin** tại thời điểm viết code. Constraint nằm ở 1 nơi, code nằm ở nơi khác. Khi context window đầy, constraint bị đẩy ra. Code tiếp tục sinh mà không biết constraint tồn tại.

**Root cause:** Trong mọi ngôn ngữ hiện tại, documentation, logic, và constraint là 3 thứ TÁCH RỜI.

---

## Giải Pháp: AIL

AIL giải quyết bằng 3 đột phá:

### 1. Code = Document = Constraint (cùng 1 thứ)

```ail
transfer money safely
  given sender is a User, receiver is a User, amount is a PositiveAmount
  producing TransferResult or InsufficientBalanceError

  requires that sender is active
  requires that receiver is active
  must always satisfy sender balance is non-negative
  guarantees that sender balance decreases by exactly the amount
```

Đọc như requirement document. **Nhưng đây là code.** Parser hiểu. Compiler verify. Transpiler emit Python.

Viết AIL = viết document. Document chạy được.

### 2. Progressive Decomposition (vô hạn layers)

Code truyền thống: 1 mức chi tiết duy nhất. 100 dòng function → AI phải nhớ hết.

AIL: vô hạn layers. Mỗi layer chỉ có 3-5 concepts. AI chỉ nghĩ 1 layer tại 1 thời điểm.

```
Layer 0: "Transfer money safely"                              ← WHAT (PO đọc được)
Layer 1: "Validate → Execute → Persist → Return"              ← WHAT phases (BA đọc được)
Layer 2: "Check sender active, check balance sufficient"       ← HOW (Dev đọc được)
Layer 3: "sender.status is 'active', otherwise raise error"    ← DETAIL (Code)
Layer 4: "UPDATE users SET balance = $1 WHERE id = $2"         ← INFRA (DBA đọc được)
```

Mỗi layer trả lời câu hỏi tiếp theo. Decompose cho đến khi máy hiểu được. Depth không giới hạn — simple project 2 layers, enterprise project 10+ layers.

### 3. CIC — Constraint Inheritance Chain (hệ thống nhớ thay AI)

**Đừng dựa vào trí nhớ AI. Hệ thống phải nhớ thay.**

```
Depth 0:  promise always: balance >= 0        ← viết 1 lần
Depth 1:  (hệ thống tự mang constraint xuống)
Depth 2:  (hệ thống tự mang constraint xuống)
Depth 5:  (hệ thống tự mang constraint xuống)  ← AI ở đây VẪN THẤY constraint
```

AI ở depth 5 không cần nhớ constraint từ depth 0. Hệ thống tính sẵn **Context Packet** — chứa TẤT CẢ constraints apply tại vị trí đó. Deterministic. Luôn đúng.

```
AI viết code vi phạm constraint?
  → ail verify BẮT NGAY
  → Trả error + counterexample + gợi ý sửa
  → AI KHÔNG THỂ ship code sai

Constraint quên = IMPOSSIBLE.
```

---

## Cách Hoạt Động

### Kiến trúc

```
.ail files → Parse → PSSD Graph → CIC Packets → Z3 Verify → Build → Python/TS + Tests
                            ↓                                           ↑
                      .ail.db (SQLite)                            dist/ or dist-ts/
                    BM25 + embeddings
```

### PSSD Graph — Code là đồ thị ngữ nghĩa

AIL dựa trên thuật toán **PSSD (Progressive Semantic Slug Discovery)** — đồ thị có hướng với 3 loại cạnh:

```
Ev (dọc):    Parent → Child       Decomposition (tổng quát → chi tiết)
Eh (ngang):  Sibling → Sibling    Sequence (bước 1 → bước 2 → bước 3)
Ed (chéo):   Node ↔ Node          Cross-reference (function ↔ type ↔ error)
```

Filesystem IS the graph:

```
src/
├── transfer_money.ail              ← Node depth 0: "Transfer money safely"
├── transfer_money/                 ← Children (Layer 1)
│   ├── validate.ail                ← Node: "Validate users"
│   ├── validate/                   ← Children (Layer 2)
│   │   ├── check_sender.ail       ← Leaf: sender must be active
│   │   └── check_balance.ail      ← Leaf: balance must be sufficient
│   ├── execute.ail                 ← Node: "Execute transfer"
│   └── persist.ail                 ← Node: "Save to database"
├── concepts/
│   ├── user.ail                    ← Type definition
│   └── wallet_balance.ail          ← Type: number >= 0
└── ail.config.toml
```

### CIC — 4 Quy Tắc Lan Truyền

```
Rule 1 — DOWN:     Parent constraints → inherit cho TẤT CẢ children, mọi depth
Rule 2 — UP:       Verified child → postcondition thành FACT cho parent
Rule 3 — ACROSS:   Previous sibling output → available cho next sibling
Rule 4 — DIAGONAL: Type constraints → auto-inject mọi nơi dùng type đó
```

### Auto-Inferred Constraints

```
Developer viết:
  define WalletBalance:number where value >= 0       ← 1 dòng

Hệ thống TỰ SUY RA ở MỌI NƠI dùng WalletBalance:
  sender.balance >= 0           ← auto
  receiver.balance >= 0         ← auto

Developer viết 5 promises. Hệ thống verify 16 constraints. 69% auto-inferred.
```

---

## Ví Dụ

### Input: transfer_money.ail

```ail
transfer money safely
  given sender is a User, receiver is a User, amount is a PositiveAmount
  producing TransferResult or InsufficientBalanceError or InvalidUserError

  requires that sender is active
  requires that receiver is active
  must always satisfy sender balance is non-negative
  guarantees that sender balance decreases by exactly the amount

  validate both users
    given sender, receiver, amount

    sender must be active,
      otherwise reject with InvalidUserError for sender
    sender balance must be at least the amount,
      otherwise reject with InsufficientBalanceError
      about current balance and requested amount

  execute the transfer
    given sender, receiver, amount

    calculate new sender balance as sender balance minus amount
    calculate new receiver balance as receiver balance plus amount

    atomically
      save sender new balance to database
      save receiver new balance to database

    respond with TransferResult containing updated sender and receiver
```

### Output: dist/transfer_money.py

```python
async def transfer_money_safely(
    sender: User, receiver: User, amount: PositiveAmount,
) -> TransferResult:
    """Transfer money safely between two users."""
    pre(sender.status == "active", "sender must be active")
    pre(receiver.status == "active", "receiver must be active")
    _old_sender_balance = sender.balance

    # --- validate both users ---
    if sender.status != "active":
        raise InvalidUserError(user_id=sender.id)
    if sender.balance < amount:
        raise InsufficientBalanceError(
            current_balance=sender.balance, requested_amount=amount)

    # --- execute the transfer ---
    new_sender_balance: WalletBalance = sender.balance - amount
    new_receiver_balance: WalletBalance = receiver.balance + amount

    async with db.transaction():
        await user_repository.update(User, {"id": sender.id}, {"balance": new_sender_balance})
        await user_repository.update(User, {"id": receiver.id}, {"balance": new_receiver_balance})

    result = TransferResult(
        sender=replace(sender, balance=new_sender_balance),
        receiver=replace(receiver, balance=new_receiver_balance),
        amount=amount)

    post(result.sender.balance == _old_sender_balance - amount)
    return result
```

**Cùng logic. AIL đọc như document. Python chạy production. Z3 đảm bảo đúng.**

---

## Reuse — 4 Cấp Độ

| Cấp               | Keyword                | Ví dụ                                                  |
| ----------------- | ---------------------- | ------------------------------------------------------ |
| **Function call** | `do X from params`     | `do deduct money from sender, amount`                  |
| **Type sharing**  | `sender:User`          | Constraints auto-inject everywhere                     |
| **Pattern reuse** | `using X where params` | `using save entity to database where entity is sender` |
| **Template**      | `following X`          | `following command flow template` — enforce structure  |

---

## CLI

**Core pipeline**

```bash
ail init <name>                          # scaffold a new project
ail build [--target python|typescript]   # full pipeline → generated source
ail build --watch                        # rebuild on file change
ail build --contracts on|comments|off    # contract emission mode
ail build --source-map                   # write functions.ailmap.json
ail build --from-db <path>               # build from a specific .ail.db
ail verify [file] [--from-db <path>]     # run pipeline without emitting
ail test                                 # build + run pytest
ail status                               # pipeline stage + graph stats
```

**Migration**

```bash
ail migrate --from <src/> --to <db> [--verify]   # filesystem → SQLite
ail export --from <db> --to <dir>                 # SQLite → export.ail
```

**Search**

```bash
ail search <query>                       # BM25 full-text (requires SQLite)
ail search <query> --semantic            # hybrid RRF (requires embeddings feature)
ail search <query> --bm25-only           # force BM25 even if --semantic given
ail search --setup                       # check ONNX model files
ail search <query> --budget <n>          # cap result count
ail reindex                              # clear embedding vectors
ail reindex --embeddings                 # rebuild embedding index
```

**CIC context**

```bash
ail context --task "<text>" [--from-db <path>]   # BM25-pick a Do node, print packet
ail context --node <name>   [--from-db <path>]   # target a node by name
```

**MCP server**

```bash
ail serve                                # start the MCP server over stdio
```

## MCP Integration

AI tools (Claude, Cursor) connect via MCP:

```json
{ "mcpServers": { "ail": { "command": "ail", "args": ["serve"] } } }
```

10 tools — 5 read: `ail.search`, `ail.context`, `ail.verify`, `ail.build`,
`ail.status`. 5 write (v2.0): `ail.write`, `ail.patch`, `ail.move`,
`ail.delete`, `ail.batch`.

---

## So Sánh

|                    | Code truyền thống     | Dafny                 | AIL                            |
| ------------------ | --------------------- | --------------------- | ------------------------------ |
| Tối ưu cho         | Developer             | Developer             | **AI**                         |
| Đọc như            | Code                  | Code + specs          | **Document tiếng Anh**         |
| Constraint nhớ bởi | Developer (quên được) | Developer (quên được) | **Hệ thống (impossible quên)** |
| Code depth         | Fixed                 | Fixed                 | **Vô hạn layers**              |
| Auto-inferred      | Không                 | Một phần              | **69% constraints tự suy ra**  |
| Output             | Chính nó              | C#/Java/Python        | **Python/TS/Rust + tests**     |

---

## Tech Stack

**Rust** (compiler) · **petgraph** (PSSD graph) · **Z3** (formal verification) · **pest** (PEG parser) · **SQLite / rusqlite · ONNX / ort · ndarray**

## Status

**v2.0.0.** All v1.0 and v2.0 phases complete:

| Phase | Deliverable | State |
|-------|-------------|-------|
| 1 | `ail-graph` — PSSD graph, CIC packets, BM25 search, validation | Done |
| 2 | `ail-types` — constraint AST, type checker, `TypedGraph` | Done |
| 3 | `ail-contract` — static checks, Z3 formal verification, `VerifiedGraph` | Done |
| 4 | `ail-text` — pest parser, deterministic `.ail` renderer | Done |
| 5a | `ail-emit` — Python emitter, pytest stubs, source maps | Done |
| 5b | `ail-mcp` + `ail-cli` — MCP server, `clap` CLI | Done |
| 6 | v1.0 polish & release — `wallet_service` fixture, docs, quality gates | Done |
| 7 | `ail-db` — SQLite backend, `GraphBackend`, FTS5, CIC cache, `ail migrate`/`ail export` | Done |
| 8 | Path-sensitive CIC — check promotion, `promoted_facts`, `ail context` | Done |
| 9 | TypeScript emitter — `dist-ts/`, branded factories, Vitest stubs | Done |
| 10 | `ail-search` — embedding provider, hybrid RRF, persisted vectors, `ail reindex` | Done |
| 11 | MCP write tools — `write`, `patch`, `move`, `delete`, `batch` (atomic) | Done |
| 12 | Integration release — `examples/wallet_service/`, e2e Python + TypeScript | Done |

End-to-end verified: `ail build` on `wallet_service` generates Python + TypeScript, all tests pass. See `CHANGELOG.md` for the full v2.0 feature list.

## License

MIT

---

_AIL — Viết document. Chạy code. Không thể sai._
