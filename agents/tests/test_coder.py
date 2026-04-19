"""15 pytest tests for ail_agent.coder — run_coder() real implementation."""

from __future__ import annotations

import copy
from typing import Any, Callable

import pytest

from ail_agent.coder import run_coder
from ail_agent.errors import AgentError, MCPConnectionError
from ail_agent.orchestrator import initial_state

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_ROOT_UUID = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
_NODE_UUID = "11111111-2222-3333-4444-555555555555"
_NODE_UUID_2 = "66666666-7777-8888-9999-aaaaaaaaaaaa"

_STEP_ROOT_PARENT: dict = {
    "pattern": "do",
    "intent": "create main handler",
    "parent_id": "root",
}

_STEP_UUID_PARENT: dict = {
    "pattern": "define",
    "intent": "define User type",
    "parent_id": _ROOT_UUID,
}

_STEP_OPTIONAL_FIELDS: dict = {
    "pattern": "do",
    "intent": "process transfer",
    "parent_id": "root",
    "expression": "transfer(amount)",
    "contracts": [{"kind": "before", "expression": "amount > 0"}],
    "metadata": {"name": "process_transfer"},
}

_STATUS_OK = {"root_id": _ROOT_UUID, "pipeline_stage": "raw", "node_count": 1, "edge_count": 0, "do_node_count": 0}
_WRITE_OK = {"status": "created", "node_id": _NODE_UUID, "depth": 1, "path": ["root intent"], "auto_edges": [], "cic_invalidated": 0, "warnings": []}
_WRITE_OK_2 = {"status": "created", "node_id": _NODE_UUID_2, "depth": 2, "path": [], "auto_edges": [], "cic_invalidated": 0, "warnings": []}


# ---------------------------------------------------------------------------
# FakeToolkit
# ---------------------------------------------------------------------------

class FakeToolkit:
    """Minimal toolkit double. Responses keyed by tool name, or as a list
    consumed sequentially, or as a callable for full control."""

    def __init__(
        self,
        responses: dict[str, Any] | list[Any] | Callable[[str, dict | None], Any],
    ) -> None:
        self._responses = responses
        self._index = 0
        self.calls: list[tuple[str, dict | None]] = []

    def call(self, tool_name: str, arguments: dict | None = None, **kw: Any) -> dict[str, Any]:
        self.calls.append((tool_name, arguments))

        if callable(self._responses):
            result = self._responses(tool_name, arguments)
        elif isinstance(self._responses, list):
            result = self._responses[self._index]
            self._index += 1
        else:
            result = self._responses[tool_name]

        if isinstance(result, Exception):
            raise result
        return result


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _make_state(**overrides: Any) -> Any:
    """Return an initial_state with a default single-step plan, then apply overrides."""
    s = initial_state("test task", steps_per_plan=20)
    s["plan"] = [_STEP_ROOT_PARENT]
    s.update(overrides)
    return s


# ---------------------------------------------------------------------------
# 1. Advances current_step on success
# ---------------------------------------------------------------------------

def test_coder_advances_current_step_on_success() -> None:
    tk = FakeToolkit({"ail.status": _STATUS_OK, "ail.write": _WRITE_OK})
    state = _make_state()
    result = run_coder(state, toolkit=tk)
    assert result["current_step"] == 1
    assert result["status"] == "code"


# ---------------------------------------------------------------------------
# 2. Routes to verify when plan is complete
# ---------------------------------------------------------------------------

def test_coder_routes_to_verify_when_plan_complete() -> None:
    tk = FakeToolkit({})
    state = _make_state(current_step=1)  # plan has 1 step, current_step==len(plan)
    result = run_coder(state, toolkit=tk)
    assert result["status"] == "verify"
    assert len(tk.calls) == 0  # no MCP calls


# ---------------------------------------------------------------------------
# 3. Step budget exceeded
# ---------------------------------------------------------------------------

def test_coder_step_budget_exceeded() -> None:
    plan = [_STEP_ROOT_PARENT] * 30
    tk = FakeToolkit({})
    state = _make_state(plan=plan, current_step=20, steps_per_plan=20)
    result = run_coder(state, toolkit=tk)
    assert result["status"] == "error"
    assert "AIL-G0143" in result["error"]
    assert "20" in result["error"]
    assert len(tk.calls) == 0


# ---------------------------------------------------------------------------
# 4. Resolves "root" via ail.status call
# ---------------------------------------------------------------------------

def test_coder_resolves_root_via_status_call() -> None:
    tk = FakeToolkit({"ail.status": _STATUS_OK, "ail.write": _WRITE_OK})
    state = _make_state()
    result = run_coder(state, toolkit=tk)
    assert result["node_id_map"]["root"] == _ROOT_UUID
    # ail.status was called with empty args
    assert ("ail.status", {}) in tk.calls


# ---------------------------------------------------------------------------
# 5. Caches root resolution — ail.status called only once across two invocations
# ---------------------------------------------------------------------------

def test_coder_caches_root_resolution() -> None:
    plan = [_STEP_ROOT_PARENT, _STEP_ROOT_PARENT]
    responses: list[Any] = [
        _STATUS_OK,         # ail.status — first invocation
        _WRITE_OK,          # ail.write — first step
        _WRITE_OK_2,        # ail.write — second step (no ail.status)
    ]
    tk = FakeToolkit(responses)
    state = _make_state(plan=plan)

    # First invocation
    state = run_coder(state, toolkit=tk)
    assert state["current_step"] == 1

    # Second invocation — root already in map
    state = run_coder(state, toolkit=tk)
    assert state["current_step"] == 2

    # ail.status must appear exactly once in the call log
    status_calls = [c for c in tk.calls if c[0] == "ail.status"]
    assert len(status_calls) == 1


# ---------------------------------------------------------------------------
# 6. Fails when ail.status returns no root_id
# ---------------------------------------------------------------------------

def test_coder_fails_when_no_root_id() -> None:
    tk = FakeToolkit({"ail.status": {}, "ail.write": _WRITE_OK})
    state = _make_state()
    result = run_coder(state, toolkit=tk)
    assert result["status"] == "error"
    assert "AIL-G0140" in result["error"]
    assert "no root node" in result["error"]


# ---------------------------------------------------------------------------
# 7. Resolves label reference from a previous step
# ---------------------------------------------------------------------------

def test_coder_resolves_label_reference() -> None:
    step0 = {
        "pattern": "do",
        "intent": "init handler",
        "parent_id": "root",
        "label": "handler",
    }
    step1 = {
        "pattern": "do",
        "intent": "add sub-step",
        "parent_id": "handler",
    }
    plan = [step0, step1]
    responses: list[Any] = [
        _STATUS_OK,   # ail.status
        _WRITE_OK,    # step0 write → node_id = _NODE_UUID
        _WRITE_OK_2,  # step1 write
    ]
    tk = FakeToolkit(responses)
    state = _make_state(plan=plan)

    # First step — creates "handler" label
    state = run_coder(state, toolkit=tk)
    assert state["node_id_map"]["handler"] == _NODE_UUID

    # Second step — parent_id="handler" should resolve to _NODE_UUID
    state = run_coder(state, toolkit=tk)
    assert state["status"] == "code"

    # Inspect the ail.write call for step1
    write_calls = [c for c in tk.calls if c[0] == "ail.write"]
    assert write_calls[1][1]["parent_id"] == _NODE_UUID


# ---------------------------------------------------------------------------
# 8. Unknown label fails
# ---------------------------------------------------------------------------

def test_coder_unknown_label_fails() -> None:
    step = {
        "pattern": "do",
        "intent": "some step",
        "parent_id": "missing",
    }
    tk = FakeToolkit({"ail.status": _STATUS_OK})
    state = _make_state(plan=[step])
    result = run_coder(state, toolkit=tk)
    assert result["status"] == "error"
    assert "unknown parent_id label" in result["error"]
    assert "missing" in result["error"]


# ---------------------------------------------------------------------------
# 9. Passes a literal UUID parent_id through unchanged (no ail.status call)
# ---------------------------------------------------------------------------

def test_coder_passes_uuid_parent_id_through() -> None:
    state = _make_state(
        plan=[_STEP_UUID_PARENT],
        node_id_map={"root": _ROOT_UUID},  # root already cached
    )
    tk = FakeToolkit({"ail.write": _WRITE_OK})
    result = run_coder(state, toolkit=tk)
    assert result["status"] == "code"
    write_calls = [c for c in tk.calls if c[0] == "ail.write"]
    assert write_calls[0][1]["parent_id"] == _ROOT_UUID
    status_calls = [c for c in tk.calls if c[0] == "ail.status"]
    assert len(status_calls) == 0


# ---------------------------------------------------------------------------
# 10. Handles MCPConnectionError from ail.write
# ---------------------------------------------------------------------------

def test_coder_handles_mcp_error() -> None:
    exc = MCPConnectionError("connect lost")
    responses: list[Any] = [_STATUS_OK, exc]
    tk = FakeToolkit(responses)
    state = _make_state()
    result = run_coder(state, toolkit=tk)
    assert result["status"] == "error"
    assert "connect lost" in result["error"]
    assert result["current_step"] == 0  # not advanced


# ---------------------------------------------------------------------------
# 11. Records label when the step has an explicit "label" field
# ---------------------------------------------------------------------------

def test_coder_records_label_when_present() -> None:
    step = {
        "pattern": "do",
        "intent": "create service",
        "parent_id": "root",
        "label": "my_step",
    }
    tk = FakeToolkit({"ail.status": _STATUS_OK, "ail.write": _WRITE_OK})
    state = _make_state(plan=[step])
    result = run_coder(state, toolkit=tk)
    assert result["node_id_map"]["my_step"] == _NODE_UUID


# ---------------------------------------------------------------------------
# 12. Falls back to intent when the step has no "label" field
# ---------------------------------------------------------------------------

def test_coder_falls_back_to_intent_when_no_label() -> None:
    step = {
        "pattern": "do",
        "intent": "some intent text",
        "parent_id": "root",
    }
    tk = FakeToolkit({"ail.status": _STATUS_OK, "ail.write": _WRITE_OK})
    state = _make_state(plan=[step])
    result = run_coder(state, toolkit=tk)
    assert result["node_id_map"]["some intent text"] == _NODE_UUID


# ---------------------------------------------------------------------------
# 13. Passes optional fields (contracts, metadata) to ail.write
# ---------------------------------------------------------------------------

def test_coder_passes_optional_fields_to_write() -> None:
    tk = FakeToolkit({"ail.status": _STATUS_OK, "ail.write": _WRITE_OK})
    state = _make_state(plan=[_STEP_OPTIONAL_FIELDS])
    result = run_coder(state, toolkit=tk)
    assert result["status"] == "code"
    write_calls = [c for c in tk.calls if c[0] == "ail.write"]
    args = write_calls[0][1]
    assert args["expression"] == "transfer(amount)"
    assert args["contracts"] == [{"kind": "before", "expression": "amount > 0"}]
    assert args["metadata"] == {"name": "process_transfer"}


# ---------------------------------------------------------------------------
# 14. Does not mutate input state
# ---------------------------------------------------------------------------

def test_coder_does_not_mutate_input_state() -> None:
    tk = FakeToolkit({"ail.status": _STATUS_OK, "ail.write": _WRITE_OK})
    state = _make_state()
    original_current_step = state["current_step"]
    original_status = state["status"]
    original_map = copy.deepcopy(state["node_id_map"])
    original_error = state["error"]

    run_coder(state, toolkit=tk)

    assert state["current_step"] == original_current_step
    assert state["status"] == original_status
    assert state["node_id_map"] == original_map
    assert state["error"] == original_error


# ---------------------------------------------------------------------------
# 15. Clears stale error on success
# ---------------------------------------------------------------------------

def test_coder_clears_error_on_success() -> None:
    tk = FakeToolkit({"ail.status": _STATUS_OK, "ail.write": _WRITE_OK})
    state = _make_state(error="stale error from previous run")
    result = run_coder(state, toolkit=tk)
    assert result["error"] == ""
    assert result["status"] == "code"


# ---------------------------------------------------------------------------
# 16. Handles missing node_id in ail.write response (regression — review #2)
# ---------------------------------------------------------------------------

def test_coder_handles_missing_node_id_in_write_response() -> None:
    """Locks Phase 14.3 review finding #2: KeyError on missing node_id must
    become an error state, not an unhandled exception."""
    # ail.write returns a dict with no node_id key.
    tk = FakeToolkit({"ail.status": _STATUS_OK, "ail.write": {}})
    state = _make_state()
    result = run_coder(state, toolkit=tk)
    assert result["status"] == "error"
    assert result["error"].startswith("[AIL-G0140]")
    assert "node_id" in result["error"]
    assert result["current_step"] == 0  # not advanced
