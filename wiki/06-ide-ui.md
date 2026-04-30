# IDE, UI và Tauri bridge

Tài liệu này bổ sung phần còn thiếu của wiki: visual IDE trong `ide/` và crate cầu nối `crates/ail-ui-bridge`. Đây là mảng v4.0 của AIL: một desktop app nơi người dùng xem graph/spec trên canvas, chạy verifier/reviewer, và chat với agent ngay trong UI.

## Bức tranh nhanh

IDE là một app **Tauri v2 + SvelteKit**:

```text
ide/ SvelteKit UI
  -> @tauri-apps/api invoke/listen
  -> ide/src-tauri standalone Cargo app
  -> ail-ui-bridge với feature tauri-commands
  -> parse/validate/type/verify AIL project
  -> GraphJson / FlowchartJson / GraphPatchJson
```

Điểm đáng nhớ:

- `ide/src-tauri/` là Cargo project riêng, không phải member của root workspace.
- `cargo build --workspace` ở root vẫn Tauri-free.
- `crates/ail-ui-bridge` mặc định không compile Tauri; command handlers chỉ bật qua feature `tauri-commands`.
- Frontend mirror schema Rust trong `ide/src/lib/types.ts`, vì vậy mọi thay đổi JSON contract cần sửa cả Rust type và TypeScript type.

## Cách chạy

Từ `ide/`:

```bash
pnpm install
pnpm run dev
pnpm tauri dev
pnpm run build
pnpm run test
pnpm run check
```

| Lệnh | Vai trò |
|---|---|
| `pnpm run dev` | Chạy Vite/SvelteKit dev server, port 1420 strict. |
| `pnpm tauri dev` | Mở desktop window đầy đủ, cần Rust toolchain. |
| `pnpm run build` | Build static frontend vào `ide/build/`. |
| `pnpm run test` | Chạy Vitest cho component/state/helper frontend. |
| `pnpm run check` | `svelte-check` với `tsconfig.json`. |

Windows build desktop cần WebView2 cho `pnpm tauri build`; `cargo check` vẫn chạy được nếu không build app desktop thật.

## Layout canonical v4

Docs v4 khóa shell canonical là:

```text
TitleBar
├─ Outline        # cây project, types, errors, filter
├─ Stage          # System / Module / Flow / Node views
└─ RightSidebar   # Chat tab luôn có, rail có thể collapse
```

Root UI nằm ở `ide/src/routes/+page.svelte`. Component chính:

| Vùng | File | Vai trò |
|---|---|---|
| TitleBar | `ide/src/lib/chrome/TitleBar.svelte` | Traffic lights, brand, breadcrumbs, lens switcher, status pills, New, Tweaks. |
| Outline | `ide/src/lib/chrome/Outline.svelte` | Project tree, filter, selection, types/errors sections, patch highlight. |
| Stage | `ide/src/lib/stage/Stage.svelte` | Dispatcher theo `selection.kind`; luôn render `LensBanner` trước view. |
| RightSidebar | `ide/src/lib/chat/RightSidebar.svelte` | Rail 44px, chat tab, dynamic sidebar slots. |
| ChatPanel | `ide/src/lib/chat/ChatPanel.svelte` | Messages, preview cards, chips, input mode, run/cancel agent. |
| Modals | `ide/src/lib/modals/*` | Welcome, Quick Create, Tweaks. |

`Stage.svelte` chọn view theo selection:

- `project` / `none` -> `SystemView`
- `module` -> `ModuleView`
- `function` -> `FlowView`
- `step` -> `NodeView`

`FlowView` có các mode như Swim, Flowchart, Code. `NodeView` là nơi đọc detail của node: code/proof/types/rules/test/history và counterexample khi verify có dữ liệu.

## State model frontend

State chung nằm ở `ide/src/lib/stores.ts`:

| Store | Ý nghĩa |
|---|---|
| `graph` | `GraphJson | null`, source of truth cho UI. |
| `selection` | Node đang chọn: project/module/function/step/type/error/none. |
| `path` | Breadcrumb path. |
| `activeLens` | Lens duy nhất: `structure`, `rules`, `verify`, `data`, `tests`. |
| `theme`, `density` | Tùy chỉnh UI session-local. |
| `quickCreateModalOpen`, `tweaksPanelOpen`, `welcomeModalOpen` | Modal state. |

Các state chuyên biệt:

- `ide/src/lib/chat/chat-state.ts`: chat mode, draft, messages, preview cards, current agent run.
- `ide/src/lib/chrome/outline-state.ts`: expanded/filter state cho Outline.
- `ide/src/lib/stage/stage-state.ts`, `flow-state.ts`, `node-view-state.ts`: view/mode local cho Stage.
- `ide/src/lib/verify/verify-state.ts`: verifier run id, running flag, `verifyTick`.

Một invariant quan trọng trong `+page.svelte`: patch graph chỉ được apply ở route-level handler. Chat preview card dispatch event lên parent; chat component không tự ghi `graph`/`selection`.

## Bridge TypeScript -> Tauri

`ide/src/lib/bridge.ts` là facade duy nhất cho frontend gọi backend:

| Function | Tauri command/event | Vai trò |
|---|---|---|
| `loadProject(path)` | `load_project` | Load project, chạy pipeline, trả `GraphJson`. |
| `getNodeDetail(nodeId)` | `get_node_detail` | Lấy detail của node từ graph đang load. |
| `getFlowchart(functionId)` | `get_flowchart` | Build `FlowchartJson` cho function. |
| `verifyProject()` | `verify_project` | Re-run pipeline đồng bộ, trả verify result MVP. |
| `computeLensMetrics(lens, scopeId)` | `compute_lens_metrics` | Tính số liệu cho `LensBanner`. |
| `startWatchProject()` | `start_watch_project` | Bật watcher `.ail` cho project hiện tại. |
| `runAgent(req)` / `cancelAgentRun(runId)` | `run_agent`, `cancel_agent_run` | Chạy/cancel Python agent sidecar. |
| `runVerifier(req)` / `cancelVerifierRun(runId)` | `run_verifier`, `cancel_verifier_run` | Chạy/cancel verifier async. |
| `onGraphUpdated` | `graph-updated` | Nhận patch khi file/watch/verifier thay đổi graph. |
| `onVerifyComplete` | `verify-complete` | Nhận kết quả verifier. |
| `onAgentStep/Message/Complete` | agent events | Stream tiến trình agent vào chat. |

Event constants mirror `crates/ail-ui-bridge/src/events.rs`.

## `ail-ui-bridge`: Rust bridge

Crate `crates/ail-ui-bridge` chuyển `VerifiedGraph` sang JSON ổn định cho UI. Entrypoint public ở `crates/ail-ui-bridge/src/lib.rs`:

- `pipeline::load_verified_from_path`: chạy parse -> validate -> type_check -> verify.
- `serialize::serialize_graph`: tạo `GraphJson`.
- `serialize::diff_graph`: tạo patch giữa hai graph.
- `lens::compute_lens_metrics`: metric theo lens/scope.
- `flowchart::build_flowchart`: dựng flowchart cho function.

Khi bật feature `tauri-commands`, crate expose command handlers:

- `load_project`
- `start_watch_project`
- `get_node_detail`
- `get_flowchart`
- `verify_project`
- `compute_lens_metrics`
- `save_flowchart`
- `run_agent`, `cancel_agent_run`
- `run_verifier`, `cancel_verifier_run`

`ide/src-tauri/src/lib.rs` chỉ wire rất mỏng:

```rust
tauri::Builder::default()
    .manage(new_bridge_state())
    .invoke_handler(get_handler())
    .run(tauri::generate_context!())
```

## BridgeState và lifecycle

`BridgeStateInner` trong `crates/ail-ui-bridge/src/commands.rs` giữ state backend:

| Field | Vai trò |
|---|---|
| `project_path` | Root project đang load. |
| `graph_json` | Graph serialized hiện tại. |
| `watcher` | File watcher `.ail`, một watcher cho mỗi project. |
| `load_generation` | Token chống race khi reload project. |
| `agent_run` | Subprocess agent đang chạy. |
| `verifier_run` | Task verifier đang chạy. |
| `*_run_seq`, `*_id_nonce` | Sinh run id dạng string để tránh lỗi precision JS. |

`load_project` làm nhiều việc cùng lúc:

1. Resolve project root và parse dir.
2. Chạy pipeline qua `load_verified_from_path`.
3. Serialize thành `GraphJson`.
4. Drop watcher cũ.
5. Cancel verifier đang chạy.
6. Tăng `load_generation`.
7. Store `project_path` và `graph_json`.

## Watcher và realtime patch

`crates/ail-ui-bridge/src/watcher.rs` theo dõi file `.ail` trong parse dir:

- debounce 250ms;
- chỉ nhận create/modify/remove trên file `.ail`;
- lọc editor temp files như `*.tmp`, `*~`, `.#*`, swap files;
- chạy pipeline ngoài lock;
- re-check `load_generation`;
- diff graph cũ/mới;
- emit `graph-updated` sau khi nhả lock.

Frontend nhận patch ở `+page.svelte`, merge bursts bằng `patch-merge.ts`, apply qua `graph-patch.ts`, tính `patchEffects`, rồi schedule verifier nếu có node bị add/modify.

`GraphPatchJson` là patch fine-grained gồm chín mảng:

- modules added/modified/removed;
- functions added/modified/removed;
- steps added/modified/removed;
- timestamp.

## Verifier integration

Verifier async nằm ở `crates/ail-ui-bridge/src/verifier.rs` và frontend scheduler ở `ide/src/lib/verify/`.

Luồng:

```text
graph-updated hoặc preview apply
  -> collect affected node ids
  -> scheduleVerify debounce
  -> run_verifier Tauri command
  -> load_verified_from_path
  -> diff fresh graph
  -> emit graph-updated nếu graph đổi
  -> emit verify-complete
  -> frontend bump verifyTick và refetch selected node detail nếu cần
```

Scope verifier hỗ trợ `project`, `module`, `function`, `step`. MVP hiện lấy failures từ `GraphJson.issues`; `VerifyFailureJson.outcome` còn `None`, còn phân loại `fail/timeout/unknown` là phần tiếp theo theo docs v4.

## Agent integration trong IDE

Agent UI dùng `ChatPanel.svelte` và backend manager `crates/ail-ui-bridge/src/agent.rs`.

Frontend gửi `AgentRunRequest` gồm:

- text người dùng;
- selection kind/id tại thời điểm gửi;
- breadcrumb path;
- active lens;
- mode: `edit`, `ask`, `test`;
- model optional.

Rust bridge spawn:

```text
python -m ail_agent "<text>" --json-events --run-id <id>
```

Sau đó reader task parse stdout line-by-line thành JSON envelope:

- `agent-step`
- `agent-message`
- `agent-complete`

Cancel guard có nhiều lớp:

1. Rust set `cancelled` flag trước.
2. Reader check run id và state lock trước khi emit.
3. `cancel_agent_run` kill child và emit complete `cancelled`.
4. Frontend listener chỉ mutate chat nếu `payload.runId === currentRunId`.

Preview card có thể chứa `GraphPatchJson`. Khi user bấm Apply, `+page.svelte` flush patch watcher đang đợi, apply preview patch, rồi schedule verifier cho affected ids.

## Lens model

`activeLens` thay thế mô hình overlay nhiều toggle. Lens hiện có:

| Lens | Mục đích UI |
|---|---|
| `structure` | Cấu trúc module/function/step/node. |
| `rules` | Contract/rule/inherited constraint. |
| `verify` | Proof, issue, counterexample, verifier status. |
| `data` | Type/dataflow/signals. |
| `tests` | Test coverage, generated test surface. |

`LensBanner.svelte` gọi `computeLensMetrics(lens, scopeId)` mỗi khi graph, lens, scope hoặc `verifyTick` đổi. Nội dung mô tả và format số liệu nằm ở `lens-banner-copy.ts`.

## System/Flow/Node UI

Các vùng stage đáng đọc:

| File | Vai trò |
|---|---|
| `SystemView.svelte` | Project-level view. |
| `SystemGrid.svelte`, `SystemGraph.svelte`, `SystemClusters.svelte` | Các mode xem toàn hệ thống. |
| `ModuleView.svelte`, `ModuleCard.svelte` | Module-level view và function rows. |
| `FlowView.svelte` | Function-level view, chọn Swim/Flowchart/Code. |
| `FlowSwim.svelte`, `SwimNode.svelte`, `FlowMinimap.svelte` | Swimlane view và minimap. |
| `FlowchartCanvas.svelte`, `FlowchartShape.svelte`, `FlowchartEdge.svelte` | SVG flowchart, shapes/edges/interaction. |
| `NodeView.svelte`, `NodeTab*.svelte` | Node detail và các tab Code/Proof/Types/Rules/Test/History. |

Hiện `Stage.svelte` vẫn có comment "Phase 16 stub; Phase 17 wires getFlowchart" cho flowchart resolution, nghĩa là một số mặt UI đã có shape/test nhưng wiring backend còn đang theo roadmap v4.

## Test coverage của IDE

Frontend có nhiều test Vitest cạnh component/helper:

- `bridge.test.ts`: Tauri facade.
- `stores.test.ts`, `patch-merge.test.ts`, `graph-patch.test.ts`: state và patch logic.
- `chat-state.test.ts`, `ChatPanel.test.ts`, `RightSidebar.test.ts`: chat/sidebar.
- `verify-state.test.ts`, `verifier-scheduler.test.ts`, `collect-scope-ids.test.ts`: verifier integration.
- `Stage.test.ts`, `SystemView.test.ts`, `FlowView.test.ts`, `FlowchartCanvas.test.ts`, `NodeView.test.ts`: stage views.
- `layout.test.ts`: root layout.

Rust bridge tests nằm trong `crates/ail-ui-bridge/tests/`:

- serialization roundtrip và graph JSON;
- fine-grained patch;
- watcher;
- verifier;
- agent;
- lens metrics;
- flowchart;
- status rollup;
- externals/relations/issues.

Lệnh thường dùng:

```bash
cargo test -p ail-ui-bridge
cd ide
pnpm run test
pnpm run check
```

## Roadmap v4 liên quan UI

Theo `docs/plan/v4.0/plan/AIL-Plan-v4.0.md`, v4 có ba phase canonical:

| Phase | Nội dung |
|---|---|
| Phase 15 | Tauri IDE Foundation: shell 3 cột, lens-driven stage, watcher, modals, design alignment. |
| Phase 16 | Agent IDE Integration: chat-to-agent stream, realtime graph patches, verifier/reviewer updates, sidecar packaging. |
| Phase 17 | Sheaf Consistency: contradiction localization, CLI/verify integration, NodeView conflict UI. |

Trạng thái kế hoạch có ghi rõ v4 đã được rebase ngày 2026-04-22; các phase cũ chỉ là archive/carryover. Khi làm việc tiếp, nên đọc docs canonical trong:

- `docs/plan/v4.0/reference/AIL-Tauri-IDE-v4.0.md`
- `docs/plan/v4.0/reference/AIL-Agent-IDE-v4.0.md`
- `docs/plan/v4.0/reference/AIL-Sheaf-Consistency-v4.0.md`
- `docs/plan/v4.0/plan/AIL-Plan-v4.0.md`

## File tham khảo nhanh

- IDE README/config: `ide/README.md`, `ide/package.json`, `ide/vite.config.ts`, `ide/src-tauri/tauri.conf.json`
- Tauri shell: `ide/src-tauri/src/lib.rs`, `ide/src-tauri/src/main.rs`
- Frontend root: `ide/src/routes/+page.svelte`
- Frontend bridge/types/stores: `ide/src/lib/bridge.ts`, `ide/src/lib/types.ts`, `ide/src/lib/stores.ts`
- Chrome/stage/chat: `ide/src/lib/chrome/`, `ide/src/lib/stage/`, `ide/src/lib/chat/`
- Patch/verify helpers: `ide/src/lib/graph-patch.ts`, `ide/src/lib/patch-merge.ts`, `ide/src/lib/verify/`
- Rust bridge: `crates/ail-ui-bridge/src/lib.rs`, `commands.rs`, `watcher.rs`, `verifier.rs`, `agent.rs`
- Bridge tests: `crates/ail-ui-bridge/tests/`

