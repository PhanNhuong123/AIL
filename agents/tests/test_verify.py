"""Tests for the verify worker (ail_agent.verify.run_verify).

Uses an inline FakeToolkit for isolation — no real MCP server is started.
The FakeToolkit records every tool name passed to .call() so tests can assert
which tools were (and were not) invoked.
"""

from __future__ import annotations

from typing import Any

import pytest

from ail_agent.errors import AgentError, MCPConnectionError
from ail_agent.orchestrator import initial_state
from ail_agent.progress import VERIFY_OK_LINE
from ail_agent.verify import run_verify


# ---------------------------------------------------------------------------
# FakeToolkit
# ---------------------------------------------------------------------------

class FakeToolkit:
    """Minimal MCPToolkit stand-in that records all .call() invocations."""

    def __init__(
        self,
        *,
        result: dict[str, Any] | None = None,
        side_effect: Exception | None = None,
    ) -> None:
        # Default to a healthy status response with 3 nodes.
        self._result = result if result is not None else {
            "pipeline_stage": "validated",
            "node_count": 3,
            "edge_count": 2,
            "do_node_count": 1,
        }
        self._side_effect = side_effect
        self.calls: list[str] = []

    def call(self, tool_name: str, arguments: dict[str, Any]) -> dict[str, Any]:  # noqa: ARG002
        self.calls.append(tool_name)
        if self._side_effect is not None:
            raise self._side_effect
        return self._result


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _base_state():
    """Return a state in the 'verify' status."""
    s = initial_state(task="add node X")
    s["status"] = "verify"
    return s


# ---------------------------------------------------------------------------
# Test 1 — LOCKED REGRESSION TEST FOR R9
# Verify worker MUST call ail.status and MUST NOT call ail.verify.
# ---------------------------------------------------------------------------

def test_verify_calls_status_not_verify_tool():
    toolkit = FakeToolkit()
    run_verify(_base_state(), toolkit=toolkit)

    assert "ail.status" in toolkit.calls, "run_verify must call ail.status"
    assert "ail.verify" not in toolkit.calls, (
        "run_verify must NOT call ail.verify — it is expensive (R9)"
    )


# ---------------------------------------------------------------------------
# Test 2 — on success the canonical VERIFY_OK_LINE is emitted exactly once.
# ---------------------------------------------------------------------------

def test_verify_emits_verify_ok_line_on_success():
    captured: list[str] = []
    toolkit = FakeToolkit()

    run_verify(_base_state(), toolkit=toolkit, emit=captured.append)

    assert captured == [VERIFY_OK_LINE]


# ---------------------------------------------------------------------------
# Test 3 — on success state["status"] is "done" and error is empty string.
# ---------------------------------------------------------------------------

def test_verify_sets_status_done_on_success():
    toolkit = FakeToolkit()
    out = run_verify(_base_state(), toolkit=toolkit)

    assert out["status"] == "done"
    assert out["error"] == ""


# ---------------------------------------------------------------------------
# Test 4 — MCPConnectionError is caught; state becomes status="error".
# ---------------------------------------------------------------------------

def test_verify_handles_mcp_connection_error():
    exc = MCPConnectionError("server not reachable", port=7777)
    toolkit = FakeToolkit(side_effect=exc)

    out = run_verify(_base_state(), toolkit=toolkit)

    assert out["status"] == "error"
    assert "server not reachable" in out["error"]


# ---------------------------------------------------------------------------
# Test 5 — AgentError (base class) is also caught correctly.
# ---------------------------------------------------------------------------

def test_verify_handles_agent_error():
    exc = AgentError("generic agent failure")
    toolkit = FakeToolkit(side_effect=exc)

    out = run_verify(_base_state(), toolkit=toolkit)

    assert out["status"] == "error"
    assert "generic agent failure" in out["error"]


# ---------------------------------------------------------------------------
# Test 6 — input state dict is never mutated.
# ---------------------------------------------------------------------------

def test_verify_does_not_mutate_input_state():
    state = _base_state()
    # Capture a deep snapshot of the values we care about.
    original_status = state["status"]
    original_error = state["error"]
    original_task = state["task"]

    toolkit = FakeToolkit()
    run_verify(state, toolkit=toolkit)

    # Original dict must be unchanged.
    assert state["status"] == original_status
    assert state["error"] == original_error
    assert state["task"] == original_task


# ---------------------------------------------------------------------------
# Test 7 — emit=None (default) does not raise.
# ---------------------------------------------------------------------------

def test_verify_emit_optional():
    toolkit = FakeToolkit()
    # Must not raise — emit defaults to None.
    out = run_verify(_base_state(), toolkit=toolkit)
    assert out["status"] == "done"


# ---------------------------------------------------------------------------
# Test 8 — zero node_count in status response triggers status="error".
# ---------------------------------------------------------------------------

def test_verify_zero_nodes_fails():
    # StatusOutput field is `node_count` (confirmed in tool_io.rs line 429).
    toolkit = FakeToolkit(result={
        "pipeline_stage": "raw",
        "node_count": 0,
        "edge_count": 0,
        "do_node_count": 0,
    })

    captured: list[str] = []
    out = run_verify(_base_state(), toolkit=toolkit, emit=captured.append)

    assert out["status"] == "error"
    assert "zero nodes" in out["error"]
    # VERIFY_OK_LINE must NOT be emitted when the check fails.
    assert captured == []
