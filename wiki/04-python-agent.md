# Python agent và runtime Python

## Vai trò trong hệ thống

Package `agents` là lớp AI agent của AIL v3.0. Nó không tự sửa file source trực tiếp; nhiệm vụ của nó là biến một yêu cầu phát triển thành các mutation trên AIL graph thông qua MCP, rồi để pipeline Rust hiện có tiếp tục chịu trách nhiệm build/verify/emit.

Luồng chính:

```text
ail agent "<task>"
  -> Rust CLI gọi Python sidecar
  -> ail_agent chọn provider/model
  -> MCPToolkit spawn `ail serve` qua stdio
  -> LangGraph chạy plan -> code -> verify
  -> graph AIL được ghi bằng MCP tool `ail.write`
  -> sanity-check bằng `ail.status`
```

Vì vậy agent Python nằm ở tầng orchestration: nó điều phối LLM, MCP và trạng thái workflow, còn logic graph, persistence, verifier Z3 và emitter vẫn nằm trong Rust crates.

## Entry point và cấu hình chạy

Entry point chính là `python -m ail_agent` hoặc script `ail-agent`, đều đi qua `agents/ail_agent/__main__.py`. File `agents/main.py` chỉ là shim để `python agents/main.py` vẫn chạy được.

CLI nhận các tham số quan trọng:

| Tham số | Mặc định | Ý nghĩa |
|---|---|---|
| `task` | bắt buộc | Mô tả việc cần agent làm. |
| `--model` | `anthropic:claude-sonnet-4-5` | Chuỗi `provider:model` để chọn adapter LLM. |
| `--max-iterations` | `50` | Giới hạn số node LangGraph được chạy để tránh loop vô hạn. |
| `--steps-per-plan` | `20` | Giới hạn số bước coder được phép thực thi trong một plan. |
| `--mcp-port` | `7777` | Đã có trong API nhưng hiện chỉ reserved; transport thực tế là stdio. |
| `--ail-bin` | `ail` | Binary dùng để spawn `ail serve`. |
| `--json-events` / `--run-id` | tắt / `0` | Bật JSON event stream cho IDE/Tauri sidecar. |

Theo `agents/README.md`, Rust CLI cũng có thể đọc `[agent]` trong `ail.config.toml`, trong đó `model`, `max_iterations`, `steps_per_plan` là các field đang active. Unknown key bị bỏ qua để giữ tương thích.

Exit code của Python side được giữ ổn định:

| Code | Ý nghĩa |
|---|---|
| `0` | Workflow kết thúc `done`. |
| `1` | Agent/workflow lỗi. |
| `2` | Model spec/config sai trước khi mở MCP. |
| `3` | Không kết nối được MCP / `ail serve`. |
| `130` | Bị ngắt bằng Ctrl-C. |

## Orchestrator: state machine LangGraph

`agents/ail_agent/orchestrator.py` định nghĩa `AILAgentState`, context injection và graph LangGraph. State được thiết kế JSON-serializable để dễ log, test và truyền qua boundary:

```text
status, task, plan, current_step, iteration,
node_id_map, error, model, mcp_port,
max_iterations, steps_per_plan
```

Các status hợp lệ là `plan`, `code`, `verify`, `done`, `error`. `route_to_agent()` route trực tiếp theo chuỗi status; node name trong LangGraph cũng chính là các chuỗi này. Nếu status lạ hoặc `None`, orchestrator không throw mà chuyển state sang `error`, ghi message `"Unknown status..."`.

`build_workflow()` tạo graph:

```text
START -> route(status)
plan   -> route(status)
code   -> route(status)
verify -> route(status)
done   -> END
error  -> END
```

Trước mỗi node `plan`, `code`, `verify`, `_enforce_iteration_limit()` tăng `iteration` và so với `max_iterations`. Nếu vượt giới hạn, state chuyển `error` với message `max_iterations (...) exceeded`.

Dependency không được import global vào worker mà được inject qua `set_workflow_context()`:

- `provider`: object thỏa `LLMProvider`;
- `model`: tên model đã tách prefix;
- `toolkit`: `MCPToolkit`;
- `emit`: callable progress.

Context này theo mô hình single-agent-per-process; tests phải gọi `clear_workflow_context()` để tránh nhiễm state.

## Planner: LLM -> plan JSON

`agents/ail_agent/planner.py` là node đầu tiên. Nó lấy `task`, tùy chọn `project_context`, rồi gọi:

```python
provider.complete(system, user, model=model)
```

Prompt nằm ở `agents/ail_agent/prompts.py` với `PROMPT_VERSION = "v3.0-2"`. Planner bắt LLM trả về duy nhất một JSON object:

```json
{"steps": [...]}
```

Mỗi step là mutation dự kiến cho graph:

| Field | Bắt buộc | Ý nghĩa |
|---|---|---|
| `pattern` | có | Một trong `always`, `check`, `define`, `describe`, `do`, `explain`, `fix`, `let`, `raise`, `set`, `test`, `use`. |
| `intent` | có | Intent tự nhiên của node. |
| `parent_id` | có | `"root"`, UUID có sẵn, hoặc label của step trước. |
| `expression` | không | Biểu thức đi kèm node. |
| `label` | không | Tên ổn định để step sau tham chiếu. |
| `contracts` | không | List `{kind, expression}` với `kind` thuộc `before`, `after`, `always`. |
| `metadata` | không | Metadata như `name`, `params`, `return_type`, `following_template_name`. |

`plan_format.parse_plan()` validate rất chặt:

- response phải là JSON object;
- top-level phải có `steps` là list không rỗng;
- mỗi step phải là object;
- field bắt buộc phải là string không rỗng;
- `pattern` và contract `kind` phải thuộc tập hợp đã định nghĩa;
- lỗi parse/validate được bọc thành `PlanError` mã `AIL-G0144`, có thể kèm `step_index` và `field_name`.

Planner không raise ra ngoài. Nếu provider hoặc parser lỗi, nó set `status = "error"` và ghi `error`; nếu thành công, nó set `plan`, reset `current_step = 0`, chuyển sang `code`.

## Coder: plan step -> MCP `ail.write`

`agents/ail_agent/coder.py` thực thi đúng một step mỗi lần node `code` được gọi. Điều này làm cho LangGraph loop dễ quan sát và budget dễ kiểm soát.

Thứ tự xử lý:

1. Nếu `current_step >= len(plan)`, coder không gọi MCP nữa mà chuyển sang `verify`.
2. Nếu `current_step >= steps_per_plan`, coder chuyển `error` với mã `AIL-G0143`.
3. Nếu chưa có `node_id_map["root"]`, coder gọi `toolkit.call("ail.status", {})` để lấy `root_id`, rồi cache lại.
4. Resolve `parent_id`:
   - `"root"` -> root UUID đã cache;
   - UUID thật -> dùng thẳng;
   - label -> lookup trong `node_id_map`, bắt buộc đã được tạo ở step trước.
5. Gọi `toolkit.call("ail.write", args)` với `parent_id`, `pattern`, `intent`, và các field optional `expression`, `contracts`, `metadata`.
6. Đọc `response["node_id"]`, lưu vào `node_id_map` theo `label` nếu có, nếu không dùng `intent`.
7. Tăng `current_step`, giữ `status = "code"` để loop tiếp.

Các lỗi MCP, thiếu root, label chưa biết, hoặc response không có `node_id` đều trở thành `status = "error"` thay vì exception rơi ra ngoài. Tests khóa call sequence quan trọng: với plan N bước, MCP đi theo mẫu `ail.status` để resolve root, sau đó `ail.write` N lần, cuối cùng verifier gọi `ail.status` thêm một lần.

## Verifier: sanity-check nhẹ, không thay thế `ail verify`

`agents/ail_agent/verify.py` hiện chỉ kiểm tra nhẹ:

- gọi `toolkit.call("ail.status", {})`;
- nếu `node_count == 0` thì lỗi;
- nếu OK thì emit dòng cố định:

```text
Basic verification passed. Run ail verify for full Z3 check.
```

Điểm quan trọng: node này không gọi MCP `ail.verify`. Full Z3 verification vẫn là trách nhiệm của lệnh Rust `ail verify`. Verify node chỉ xác nhận graph vẫn load được và không rỗng sau khi coder ghi.

## MCPToolkit: cầu nối sync Python -> async MCP stdio

`agents/ail_agent/mcp_toolkit.py` bọc MCP SDK async thành API sync để các node LangGraph có thể là function đồng bộ.

Khi vào context manager:

- tạo event loop riêng;
- chạy loop trên daemon thread;
- spawn server bằng `stdio_client(StdioServerParameters(command="ail", args=["serve"]))`;
- mở `ClientSession` và `initialize()`;
- timeout connect mặc định `5s`.

`MCPToolkit.call(tool_name, arguments)` submit coroutine bằng `asyncio.run_coroutine_threadsafe()`, block chờ result với timeout mặc định `30s`, rồi parse các text content block thành JSON dict. Nếu response không phải JSON object, toolkit raise `MCPConnectionError`.

`close()` idempotent: đóng `AsyncExitStack`, clear session và stop event loop. Tests kiểm tra timeout connect/call, call sau close, list tools, non-JSON response và context manager cleanup.

## Provider registry và provider swap

Provider layer thống nhất quanh protocol `LLMProvider` trong `agents/ail_agent/providers/base.py`:

```python
complete(system, user, *, model) -> str
complete_with_tools(system, user, *, model, tools, tool_choice=None) -> CompletionResult
```

`CompletionResult` normalize output thành:

```text
text: str | None
tool_calls: list[{id, name, arguments}]
```

`registry.parse_model_spec()` yêu cầu format `"<provider>:<model>"`; prefix được lowercase, model giữ nguyên. `get_provider()` lazy-import provider class, vì vậy thiếu SDK optional chỉ lỗi khi provider đó được dùng.

Provider hiện có:

| Prefix | Provider class | Env/config | Ghi chú |
|---|---|---|---|
| `anthropic` | `AnthropicProvider` | `ANTHROPIC_API_KEY` | Default CLI là `anthropic:claude-sonnet-4-5`; dùng SDK Anthropic. |
| `openai` | `OpenAIProvider` | `OPENAI_API_KEY` | Dùng SDK OpenAI native. |
| `deepseek` | `DeepSeekProvider` | `DEEPSEEK_API_KEY`, base URL `https://api.deepseek.com` | OpenAI-compatible; `deepseek-reasoner` dùng prompt fallback cho tools. |
| `alibaba` | `AlibabaProvider` | `DASHSCOPE_API_KEY`, DashScope compatible URL | OpenAI-compatible; `qwen-turbo` dùng prompt fallback. |
| `qwen` | alias | `DASHSCOPE_API_KEY` | Alias về `AlibabaProvider`, provider name vẫn là `alibaba`. |
| `ollama` | `OllamaProvider` | `OLLAMA_BASE_URL` hoặc `http://localhost:11434` | Local HTTP `/api/chat`; không cần API key. |

Provider swap chỉ là đổi `--model`. Tests chứng minh hai lần gọi `main()` liên tiếp với model khác nhau vẫn thành công trong cùng process, và workflow không hard-code provider name.

## Retry và lỗi

Error hierarchy nằm trong `agents/ail_agent/errors.py`, dùng dải mã `AIL-G014x`:

| Error | Mã | Khi nào |
|---|---|---|
| `AgentError` | `AIL-G0140` | Base class cho agent-layer errors. |
| `ProviderError` | `AIL-G0140` | Provider call lỗi hoặc retry hết lượt. |
| `ProviderConfigError` | `AIL-G0140` | Thiếu env var, model spec sai, prefix lạ. |
| `RoutingError` | `AIL-G0141` | Reserved, chưa wire vào orchestrator. |
| `StepBudgetError` | `AIL-G0143` | Budget step bị vượt. Trong coder hiện lỗi được set trực tiếp vào state. |
| `PlanError` | `AIL-G0144` | LLM plan JSON sai format/schema. |
| `MCPConnectionError` | `AIL-G0145` | Không kết nối/gọi được MCP hoặc response MCP không hợp lệ. |

`providers/_retry.py` có `with_retry()` viết tay với delay mặc định `(1.0, 2.0, 4.0)`, tức tối đa 4 attempt gồm lần đầu. Chỉ exception được classifier `is_transient` đánh dấu mới retry; lỗi permanent propagate ngay. Sau khi hết lượt, nó raise `ProviderError("exhausted 4 attempts", cause=last_exc)`.

Transient policy:

- Anthropic: connection, timeout, rate limit, HTTP 5xx.
- OpenAI-compatible: `APIConnectionError`, `APITimeoutError`, `RateLimitError`, HTTP 5xx.
- Ollama/httpx: timeout, connect error, HTTP 5xx.

Các SDK client đọc env var ở lúc call `_client()`, không đọc ở module import hoặc constructor. Điều này giúp tests và provider registry side-effect free hơn.

## Progress output: text và JSON events

`agents/ail_agent/progress.py` có hai emitter:

- `Progress`: output text line-by-line, flush ngay. Đây là format tương thích CLI cũ.
- `JsonProgress`: output mỗi dòng một JSON envelope cho Tauri IDE sidecar.

Envelope JSON có ba nhóm:

```json
{"type":"step","runId":"...","index":1,"phase":"plan","text":"Planning..."}
{"type":"message","runId":"...","messageId":"...","text":"...","preview":null}
{"type":"complete","runId":"...","status":"done","error":null}
```

`JsonProgress` tự tăng `index`, giữ `phase` là `plan`, `code` hoặc `verify`, và flush từng dòng để Rust reader nhận event incremental. Traceback/warning Python vẫn đi stderr theo mặc định.

## Runtime Python cho code được emit

`crates/ail-runtime-py` là package `ail-runtime`, runtime helper dành cho Python code do AIL emitter sinh ra. Nó độc lập với `agents`: agent dùng để ghi graph, còn runtime được import bởi generated code sau emit.

Public API trong `ail_runtime/__init__.py` export:

- contract helpers: `pre`, `post`, `keep`, `ContractViolation`;
- builtin validators: `AilType`, `PositiveInteger`, `NonNegativeInteger`, `PositiveAmount`, `Percentage`, `NonEmptyText`, `EmailAddress`, `Identifier`;
- repository interfaces: `AilRepository`, `AsyncAilRepository`;
- `__version__`.

### Guards và contracts

`ail_runtime/contracts.py` định nghĩa:

```python
pre(condition, message="")
post(condition, message="")
keep(condition, message="")
```

Nếu condition false, chúng raise `ContractViolation` với message lần lượt là `pre-condition violated`, `post-condition violated`, hoặc `invariant violated`. Đây là runtime guard đơn giản cho code sinh ra; static/Z3 verification vẫn ở pipeline Rust.

### Builtin type validators

`ail_runtime/types.py` định nghĩa base abstract `AilType` với:

```python
validate(value) -> bool
assert_valid(value) -> None
```

`assert_valid()` raise `ContractViolation` nếu `validate()` trả false.

Các validator hiện có:

| Type | Rule |
|---|---|
| `PositiveInteger` | `int > 0`, loại `bool`, không nhận `float` dù là `5.0`. |
| `NonNegativeInteger` | `int >= 0`, loại `bool` và `float`. |
| `PositiveAmount` | `int` hoặc `float` hữu hạn và `> 0`, loại `bool`, `NaN`, infinity. |
| `Percentage` | `int` hoặc `float` hữu hạn trong `[0, 100]`, loại `bool`, `NaN`, infinity. |
| `NonEmptyText` | `str` có `strip()` không rỗng. |
| `EmailAddress` | Regex cơ bản `local@domain.tld`, khớp semantic type Rust. |
| `Identifier` | Regex `^[a-zA-Z_][a-zA-Z0-9_]*$`, khớp semantic type Rust. |

### Repository contract

`ail_runtime/repository.py` cung cấp hai abstract base class cho code data access do emitter sinh ra:

- `AilRepository`: sync, dùng khi `EmitConfig.async_mode = false`;
- `AsyncAilRepository`: async, dùng khi `EmitConfig.async_mode = true`.

Cả hai đều yêu cầu đủ bốn method:

```python
get(entity_type, condition)
save(entity)
update(entity_type, condition, fields)
delete(entity_type, condition)
```

Generated fetch/save/update/remove pattern gọi interface này mà không truyền source name; source routing là trách nhiệm của developer khi instantiate repository cụ thể.

## Test coverage nên đọc

Các test trong `agents/tests` mô tả contract hành vi khá rõ:

- `test_orchestrator.py`: state JSON-safe, routing, iteration guard, context injection, LangGraph termination.
- `test_planner.py` và `test_plan_format.py`: planner prompt output, parse lỗi JSON/schema, contract kind hợp lệ.
- `test_coder.py`: root resolution, label/UUID parent, step budget, optional fields, no mutation input state.
- `test_verify.py`: verify chỉ gọi `ail.status`, không gọi `ail.verify`, emit dòng verify OK.
- `test_mcp_toolkit.py`: stdio MCP wrapper, timeout, JSON extraction, close idempotent.
- `test_provider_swap.py` và `test_providers.py`: registry, env var, OpenAI-compatible fallback, Ollama native/prompt tools, provider-agnostic workflow.
- `test_progress_json.py`: shape JSON events và flush từng dòng.
- `test_workflow_e2e_mocked.py` / `test_workflow_e2e_wallet.py`: happy path nhiều step, budget, MCP call sequence wallet-service.

Runtime Python có test riêng trong `crates/ail-runtime-py/tests` cho contract helpers, validators và abstract repository classes.

## Những điểm cần nhớ khi phát triển tiếp

- Agent Python không nên bypass MCP để sửa graph hoặc source trực tiếp; MCP/Rust là source of truth.
- `AILAgentState` cần giữ JSON-serializable.
- Provider mới chỉ cần implement `LLMProvider` và đăng ký prefix trong `registry.py`.
- Nếu model không support native tools, fallback phải normalize về cùng `CompletionResult`.
- `run_verify()` hiện là sanity-check nhẹ; tài liệu/UI phải nói rõ người dùng vẫn nên chạy `ail verify`.
- `MCPToolkit` hiện dùng stdio dù có `mcp_port`; đừng giả định network transport đã active.
- Runtime Python phục vụ generated code, không phải runtime của chính agent.
