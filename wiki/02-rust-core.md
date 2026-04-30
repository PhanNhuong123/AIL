# Rust core AIL

Tài liệu này mô tả phần Rust core của AIL trong workspace `Cargo.toml`: các crate chính trong `crates/`, pipeline dữ liệu, các stage gate kiểu Rust, và những bề mặt phục vụ CLI/MCP/IDE. Mã Python agent và frontend IDE nằm ngoài phạm vi trực tiếp của core, nhưng `ail-cli`, `ail-mcp` và `ail-ui-bridge` là các cổng Rust nối core với chúng.

## 1. Bức tranh tổng quan

AIL tổ chức core theo pipeline một chiều:

```text
.ail text / .ail.db
  -> AilGraph
  -> ValidGraph
  -> TypedGraph
  -> VerifiedGraph
  -> generated files / MCP responses / IDE JSON
```

Stage gate là kiểu dữ liệu, không chỉ là convention:

| Stage | Crate sở hữu | Kiểu/entrypoint | Ý nghĩa |
|---|---|---|---|
| Parse/spec | `ail-text`, `ail-db` | `parse`, `parse_directory`, `SqliteGraph` | Đọc `.ail` hoặc SQLite thành graph thô. |
| Graph | `ail-graph` | `AilGraph`, `GraphBackend` | Mô hình PSSD: node, edge, metadata, contract. |
| Validate | `ail-graph` | `validate_graph -> ValidGraph` | Kiểm tra cấu trúc cây, reachability, type refs sơ bộ, template/use rules. |
| Type | `ail-types` | `type_check -> TypedGraph` | Resolve type, field access, data flow, call param typing. |
| Verify | `ail-contract` | `verify -> VerifiedGraph` | Static contract checks; Z3 nếu bật feature `z3-verify`; sheaf/Čech analysis. |
| Emit/serve | `ail-emit`, `ail-cli`, `ail-mcp`, `ail-ui-bridge` | emitters, commands, JSON-RPC/Tauri handlers | Chỉ tiêu thụ graph đã qua gate phù hợp. |

Các entrypoint pipeline thật đang lặp lại cùng thứ tự này:

- CLI build/verify: `crates/ail-cli/src/commands/build.rs`, `crates/ail-cli/src/commands/verify.rs`
- MCP refresh: `crates/ail-mcp/src/pipeline.rs`
- UI bridge load: `crates/ail-ui-bridge/src/pipeline.rs`

## 2. Dependency nội bộ

Workspace khai báo crate nội bộ ở `Cargo.toml`. Quan hệ chính:

| Crate | Phụ thuộc nội bộ | Vai trò chính |
|---|---|---|
| `ail-graph` | không phụ thuộc crate AIL khác | Nền graph, validation, CIC, BM25, folder index. |
| `ail-db` | `ail-graph` | SQLite backend cho `GraphBackend`, FTS5, cache CIC/coverage/embedding. |
| `ail-types` | `ail-graph` | AST constraint/value, type checker, evaluator, semantic builtins. |
| `ail-contract` | `ail-graph`, `ail-types` | Static contract checks, Z3 encode/verify, `VerifiedGraph`, sheaf. |
| `ail-text` | `ail-graph`, `ail-types` | Parser `.ail` bằng pest, renderer text. |
| `ail-emit` | `ail-graph`, `ail-types`, `ail-contract` | Emit Python/TypeScript từ `VerifiedGraph`. |
| `ail-search` | `ail-graph` | BM25+embedding hybrid search, ONNX provider tùy feature. |
| `ail-coverage` | `ail-graph`, `ail-search` | Semantic coverage parent->children. |
| `ail-mcp` | graph/types/contract/text/emit/search; optional db/coverage | JSON-RPC MCP tools cho agent. |
| `ail-cli` | hầu hết các crate core | Binary `ail`: build, verify, search, coverage, sheaf, agent... |
| `ail-ui-bridge` | graph/types/contract/text | Stable JSON bridge và Tauri commands cho IDE. |

Feature gates đáng chú ý:

- `ail-contract/z3-verify`: bật Z3 encode/verify và obstruction detector.
- `ail-search/embeddings`: bật ONNX Runtime + tokenizer cho semantic embeddings.
- `ail-mcp/embeddings`, `ail-cli/embeddings`: nối semantic search/coverage vào tool surfaces.
- `ail-ui-bridge/tauri-commands`: bật Tauri, watcher, verifier async, agent subprocess.

## 3. Graph core và CIC

`ail-graph` là stage nền: `crates/ail-graph/src/lib.rs`.

Các module quan trọng:

- `types/`: `Node`, `NodeId`, `Pattern`, `EdgeKind`, `Contract`.
- `graph/`: `AilGraph`, `AilGraphBuilder`, `GraphBackend`, navigation.
- `validation/`: `validate_graph`, `ValidGraph`, structural rules.
- `cic/`: Context Packet và bốn rule propagation.
- `search/`: BM25 in-memory search.
- `index/`: folder index/name resolver.

Mô hình graph dùng 17 pattern đóng trong `crates/ail-graph/src/types/pattern.rs`: `Define`, `Describe`, `Error`, `Do`, `Promise`, `Let`, `Check`, `ForEach`, `Match`, `Fetch`, `Save`, `Update`, `Remove`, `Return`, `Raise`, `Together`, `Retry`.

Ba loại edge nằm trong `crates/ail-graph/src/types/edge.rs`:

| Edge | Hướng | Ý nghĩa |
|---|---|---|
| `Ev` | parent -> child | Cây cấu trúc/decomposition. |
| `Eh` | sibling -> sibling | Thứ tự thực thi hoặc thứ tự file. |
| `Ed` | cross-reference | Type/error/function/template/shared pattern references. |

`GraphBackend` trong `crates/ail-graph/src/graph/backend.rs` là abstraction chung cho in-memory `AilGraph` và SQLite `SqliteGraph`. Downstream nhận `&dyn GraphBackend` để tránh gắn chặt vào storage.

CIC nằm ở `crates/ail-graph/src/cic/mod.rs`. Nó tạo `ContextPacket` backend-agnostic bằng `compute_context_packet_for_backend`:

- DOWN: contract từ ancestor chảy xuống `inherited_constraints`.
- UP: postcondition đã verify của child chuẩn bị thành parent facts.
- ACROSS: output/scope/promoted facts từ sibling trước chảy sang sibling sau, có xét ancestor levels.
- DIAGONAL: type constraints và call/template/shared-pattern contract chảy qua `Ed`.

`ail-db` dùng cùng thuật toán CIC, nhưng thêm cache trong `crates/ail-db/src/db/cic_cache.rs`. Invalidation cũng theo bốn hướng: descendants, ancestors, next siblings + descendants, incoming `ed`; riêng `Check` còn lan promotion đến các node sau.

## 4. Text parser: spec -> graph

`ail-text` là lớp `.ail` text: `crates/ail-text/src/lib.rs`.

Pipeline parser:

```text
source text
  -> pest grammar (`grammar.pest`)
  -> walker (`ParsedStatement`)
  -> assembler (`AilGraph`)
  -> validate_graph
```

Các file chính:

- `crates/ail-text/src/grammar.pest`: grammar PEG cho 17 pattern, có synonym tiếng Anh/document-style.
- `crates/ail-text/src/parser/walker.rs`: chuyển pest pairs thành `ParsedStatement`, metadata, raw expression, contract.
- `crates/ail-text/src/parser/assembler.rs`: dựng node, Ev/Eh edges theo indentation, gắn `promise` vào parent `Do`, resolve `following`/`using` thành `Ed`.
- `crates/ail-text/src/parser/directory.rs`: đi cây thư mục, tạo container node, parse mọi `.ail`, nối Ev/Eh theo thứ tự deterministic.
- `crates/ail-text/src/renderer/`: render graph trở lại `.ail` text.

Điểm quan trọng: grammar chỉ đảm bảo shape/pattern; nhiều expression được giữ dạng raw text. Syntax/semantic của constraint/value đi qua `ail-types` và `ail-contract`.

## 5. Type system: graph -> typed

`ail-types` là gate thứ hai: `crates/ail-types/src/lib.rs`.

Nó sở hữu:

- Constraint/value AST: `ConstraintExpr`, `ValueExpr` trong `crates/ail-types/src/types/`.
- Parser expression: `parse_constraint_expr`, `parse_value_expr` trong `crates/ail-types/src/expr/`.
- Builtin semantic types: `crates/ail-types/src/builtins/semantic_types.rs`.
- Runtime-style evaluator: `eval_constraint`, `eval_value`.
- Type checker: `crates/ail-types/src/checker/mod.rs`.

`type_check(valid, packets)` chạy bốn nhóm check:

1. Resolve mọi `type_ref` trong params, return type, base type, fields, carries.
2. Parse contract expression và kiểm tra dotted field access như `sender.balance`.
3. Kiểm tra data-flow type từ CIC packet: scope variables, `must_produce`, return type.
4. Kiểm tra param types qua outgoing `Ed` khi node gọi một `Do` khác.

Nếu caller không truyền `ContextPacket`, `type_check` tự tính packet cho mọi node qua `compute_context_packet_for_backend`. Output `TypedGraph` wrap `ValidGraph`, nên stage sau vẫn truy cập graph qua `graph()` hoặc `ail_graph()`.

## 6. Contract verification, Z3 và sheaf

`ail-contract` là gate verify: `crates/ail-contract/src/lib.rs`, entrypoint `verify` ở `crates/ail-contract/src/verify.rs`.

`verify(typed)` luôn chạy static checks trước:

- Before-contract scope: không dùng `old()`, chỉ input params.
- After-contract scope: params + `result`.
- Raise refs: error phải được declared bởi enclosing `Do`.
- Template phase coverage.
- Contract expression parse errors.

Nếu static check có lỗi, Z3 không chạy.

Khi bật feature `z3-verify`, `crates/ail-contract/src/z3_verify/mod.rs` chạy Z3 trên mọi `Do` node theo thứ tự bottom-up:

1. Dựng `EncodeContext` từ params, return type và scalar fields (`context_builder.rs`).
2. Assert type constraints từ semantic builtins như `PositiveInteger`, `NonNegativeInteger`, `PositiveAmount`, `Percentage`.
3. Assert verified child postconditions để compositional verification.
4. Assert `Before` contracts và kiểm tra SAT.
5. Assert promoted facts từ preceding `check ... otherwise raise ...`; nếu mâu thuẫn thì báo `PromotedFactContradiction`.
6. Với từng `After`/`Always`, assert `not(post)`; UNSAT nghĩa là postcondition được chứng minh, SAT trả counterexample từ model.

Z3 encode nằm ở `crates/ail-contract/src/z3_encode/`: `EncodeContext`, `encode_constraint`, `encode_value_*`, `encode_type_constraint`. Solver timeout hiện đặt 30 giây qua `z3::Config`.

Sheaf/Cech nằm ở `crates/ail-contract/src/sheaf/`:

- `build_nerve(VerifiedGraph) -> CechNerve`
- chỉ tạo section cho `Pattern::Do`
- overlap trực tiếp parent-child và sibling share biến
- deterministic sort theo `NodeId`
- `filter_to_subtree` phục vụ CLI scope

Khi bật `z3-verify`, `analyze_sheaf_obstructions` và `detect_obstructions` dùng Z3 để phân loại overlap thành consistent/contradictory/unknown. CLI `ail verify` có augmentation cho UNSAT class errors trong `crates/ail-cli/src/commands/verify.rs`; CLI riêng `ail sheaf` nằm ở `crates/ail-cli/src/commands/sheaf/`.

## 7. Emitters: verified -> output

`ail-emit` chỉ nhận `VerifiedGraph`: `crates/ail-emit/src/lib.rs`.

Python target:

- `emit_type_definitions`: sinh `generated/types.py` từ `Define`, `Describe`, `Error`.
- `emit_function_definitions`: sinh `generated/functions.py`, `generated/test_contracts.py`, `generated/functions.ailmap.json`.
- `emit_scaffold_files`: sinh scaffold developer-owned, chỉ write-once.
- `contract_inject.rs`: render `Before`/`Always` assertions trước body, `After` sau body, hỗ trợ snapshot `old(...)`.

TypeScript target:

- `emit_ts_type_definitions`: branded types, interfaces, Error subclasses.
- `emit_ts_function_definitions`: function files, repo interfaces, barrel exports.
- `emit_ts_test_definitions`: Vitest/Jest `it.todo()` stubs.
- `emit_ts_project_files`: project support files.

`EmitConfig` trong `crates/ail-emit/src/types/emit_config.rs` điều khiển `async_mode`, `contract_mode` (`On`, `Comments`, `Off`, `Test`) và TS test framework. `EmittedFile` có `FileOwnership::Generated` hoặc `Scaffolded` để CLI biết file nào được overwrite.

## 8. SQLite, search và coverage

`ail-db` cung cấp `SqliteGraph`: `crates/ail-db/src/lib.rs`, `crates/ail-db/src/db/sqlite_graph.rs`.

Schema chính ở `crates/ail-db/src/db/schema.rs`:

- `nodes`, `contracts`, `edges`, `project_meta`
- `cic_cache`
- `embeddings`
- `coverage_cache`
- FTS5 virtual table `search_fts` với trigger sync từ `nodes`

`SqliteGraph` bật WAL, foreign keys và synchronous normal khi mở connection. Nó implement `GraphBackend`, có `save_from_graph` để flush in-memory graph về DB, FTS5 search, embedding persistence và coverage cache.

`ail-search` nằm ở `crates/ail-search/src/lib.rs`:

- BM25 có sẵn từ `ail-graph::Bm25Index` hoặc FTS5 từ `ail-db`.
- `EmbeddingProvider` là trait provider-agnostic.
- `EmbeddingIndex` giữ vector in-memory và dùng `node_embedding_text(node)` làm canonical text.
- `hybrid_search` hợp nhất BM25 + semantic bằng Reciprocal Rank Fusion.
- Optional `OnnxEmbeddingProvider` dùng `all-MiniLM-L6-v2` qua ONNX Runtime.

`ail-coverage` nằm ở `crates/ail-coverage/src/lib.rs`. `compute_coverage` embed parent + children, dựng orthonormal basis bằng Modified Gram-Schmidt, project parent intent lên span của children, rồi suy ra:

- score `[0, 1]` hoặc `None` cho leaf.
- child contributions.
- missing aspects từ residual direction so với concept list.
- fallback/guard cho zero vector hoặc degenerate basis.

## 9. CLI, MCP và UI bridge

`ail-cli` là binary `ail`, nhưng phần logic testable nằm ở `crates/ail-cli/src/lib.rs`.

Các command quan trọng:

- `build`: resolve backend, load graph, validate/type/verify, emit Python hoặc TypeScript.
- `verify`: full pipeline, optional sheaf localization với `z3-verify`.
- `context`: CIC context packet.
- `search`/`reindex`: BM25 hoặc semantic embedding.
- `coverage`: single node, all, warm-cache.
- `migrate`/`export`: filesystem `.ail` <-> `.ail.db`.
- `serve`: MCP stdio.
- `sheaf`: Cech nerve/obstruction output text/json.
- `agent`: spawn Python LangGraph agent package.

Backend resolution ở `crates/ail-cli/src/commands/project.rs`: `--from-db` thắng, sau đó `[database] backend = "sqlite" | "filesystem" | "auto"` trong `ail.config.toml`, cuối cùng fallback filesystem.

`ail-mcp` bọc core thành JSON-RPC MCP server: `crates/ail-mcp/src/lib.rs`, `crates/ail-mcp/src/server.rs`.

Tool hiện có:

- Read/build: `ail.search`, `ail.review`, `ail.context`, `ail.verify`, `ail.build`, `ail.status`
- Write/mutation: `ail.write`, `ail.patch`, `ail.move`, `ail.delete`, `ail.batch`

`ProjectContext` trong `crates/ail-mcp/src/context.rs` giữ stage cao nhất (`Raw`, `Valid`, `Typed`, `Verified`). Khi write tool lấy mutable graph, context bị demote về `Raw`, đánh dấu dirty, clear search/embedding cache; verify/build sau đó refresh từ in-memory graph để không mất edit.

`ail-ui-bridge` là cầu nối Rust -> Tauri/Svelte IDE: `crates/ail-ui-bridge/src/lib.rs`.

Module chính:

- `pipeline.rs`: `load_verified_from_path` chạy parse -> validate -> type_check -> verify.
- `serialize/`: `serialize_graph`, `diff_graph`, stable `GraphJson`/`GraphPatchJson`.
- `ids.rs`: path-like stable IDs cho frontend.
- `rollup.rs`: rollup status theo children.
- `lens/`: compute metrics cho lens `structure | rules | verify | data | tests`.
- `flowchart.rs`: build flowchart JSON cho function.
- `commands.rs`: Tauri command handlers.
- `watcher.rs`: file watcher `.ail`, debounced diff + graph-updated event.
- `verifier.rs`: async verifier run/cancel, `verify-complete`.
- `agent.rs`: spawn/cancel Python agent subprocess, stream JSON events.

Default build không kéo Tauri; mọi command runtime nằm sau feature `tauri-commands`.

## 10. Đường đi điển hình

Build từ filesystem:

```text
ail build
  -> resolve_backend
  -> parse_directory(src hoặc project root)
  -> validate_graph
  -> type_check
  -> verify
  -> emit_* target
  -> write generated/scaffolded files
```

MCP write rồi verify:

```text
ail.write / ail.patch / ail.move / ail.delete
  -> ProjectContext::graph_mut demote stage về Raw
  -> mutate AilGraph
  -> dirty = true, clear caches
ail.verify
  -> refresh_from_graph
  -> validate_graph
  -> type_check
  -> verify
```

IDE load/watch:

```text
load_project
  -> resolve project layout
  -> load_verified_from_path(parse_dir)
  -> serialize_graph -> GraphJson
start_watch_project
  -> debounce .ail changes
  -> rerun pipeline
  -> diff_graph
  -> emit graph-updated patch
```

Coverage/search path:

```text
graph node text
  -> BM25 / FTS5 keyword search
  -> optional ONNX embeddings
  -> hybrid RRF search
  -> coverage projection and missing-aspect detection
  -> persisted in SQLite coverage_cache when caller chooses
```

## 11. File tham khảo nhanh

- Workspace/deps: `Cargo.toml`
- Repo status/pipeline overview: `README.md`, `CLAUDE.md`, `crates/CLAUDE.md`
- Graph core: `crates/ail-graph/src/lib.rs`, `crates/ail-graph/src/graph/backend.rs`, `crates/ail-graph/src/cic/mod.rs`, `crates/ail-graph/src/validation/rules.rs`
- Parser: `crates/ail-text/src/lib.rs`, `crates/ail-text/src/grammar.pest`, `crates/ail-text/src/parser/assembler.rs`, `crates/ail-text/src/parser/directory.rs`
- Type checker: `crates/ail-types/src/lib.rs`, `crates/ail-types/src/checker/mod.rs`, `crates/ail-types/src/checker/checks.rs`
- Verification/Z3/sheaf: `crates/ail-contract/src/lib.rs`, `crates/ail-contract/src/verify.rs`, `crates/ail-contract/src/z3_verify/mod.rs`, `crates/ail-contract/src/z3_encode/mod.rs`, `crates/ail-contract/src/sheaf/mod.rs`
- Emitters: `crates/ail-emit/src/lib.rs`, `crates/ail-emit/src/python/emit_functions.rs`, `crates/ail-emit/src/typescript/mod.rs`
- DB/search/coverage: `crates/ail-db/src/db/schema.rs`, `crates/ail-search/src/hybrid.rs`, `crates/ail-coverage/src/coverage.rs`
- CLI/MCP/UI: `crates/ail-cli/src/lib.rs`, `crates/ail-mcp/src/server.rs`, `crates/ail-ui-bridge/src/lib.rs`, `crates/ail-ui-bridge/src/commands.rs`
