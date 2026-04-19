"""End-to-end workflow tests with FakeProvider and FakeToolkit (task 14.3 step 13).

These tests run the FULL build_workflow() LangGraph graph with real workers
(planner, coder, verify) but replace the LLM provider and MCP toolkit with
in-process fakes so no network or subprocess calls happen.
"""
from __future__ import annotations

import json
from typing import Any, Callable

import pytest

from ail_agent.errors import MCPConnectionError
from ail_agent.orchestrator import (
    AILAgentState,
    build_workflow,
    clear_workflow_context,
    initial_state,
    set_workflow_context,
)
from ail_agent.providers.base import CompletionResult, LLMProvider, ToolCall, ToolSpec

# ---------------------------------------------------------------------------
# Fakes
# ---------------------------------------------------------------------------

_ROOT_ID = "aaaaaaaa-0000-0000-0000-000000000001"


class FakeProvider:
    """Returns a canned plan JSON string from .complete().

    Set plan_steps before calling build_workflow to control what the planner
    returns.
    """

    name: str = "fake"

    def __init__(self, plan_steps: list[dict[str, Any]]) -> None:
        self._plan_json = json.dumps({"steps": plan_steps})

    def complete(self, system: str, user: str, *, model: str) -> str:
        return self._plan_json

    def complete_with_tools(
        self,
        system: str,
        user: str,
        *,
        model: str,
        tools: list[ToolSpec],
        tool_choice: str | None = None,
    ) -> CompletionResult:
        return CompletionResult(text=self._plan_json, tool_calls=[])


class FakeToolkit:
    """Returns canned responses for ail.status and ail.write.

    Call sequence for a plan with N steps:
      - First call to ail.status (root resolution) → {"root_id": ROOT_ID, "node_count": 1}
      - Each ail.write → {"node_id": "<generated>"}
      - Final ail.status (verify) → {"root_id": ROOT_ID, "node_count": N+1}
    """

    def __init__(self, *, fail_on_write: bool = False) -> None:
        self._write_counter = 0
        self._fail_on_write = fail_on_write
        self.calls: list[tuple[str, dict]] = []

    def call(self, tool_name: str, arguments: dict[str, Any] | None = None) -> dict[str, Any]:
        args = arguments or {}
        self.calls.append((tool_name, args))
        if tool_name == "ail.status":
            return {"root_id": _ROOT_ID, "node_count": self._write_counter + 1}
        if tool_name == "ail.write":
            if self._fail_on_write:
                raise MCPConnectionError("simulated write failure")
            self._write_counter += 1
            return {"node_id": f"bbbbbbbb-0000-0000-0000-{self._write_counter:012d}"}
        raise MCPConnectionError(f"unexpected tool call: {tool_name!r}")


# ---------------------------------------------------------------------------
# Fixture: reset workflow context between tests
# ---------------------------------------------------------------------------

@pytest.fixture(autouse=True)
def reset_context():
    clear_workflow_context()
    yield
    clear_workflow_context()


# ---------------------------------------------------------------------------
# Helper
# ---------------------------------------------------------------------------

def _run(
    task: str,
    plan_steps: list[dict[str, Any]],
    *,
    toolkit: FakeToolkit | None = None,
    max_iterations: int = 50,
    steps_per_plan: int = 20,
    recursion_limit: int = 200,
) -> AILAgentState:
    """Wire context, build graph, invoke, and return final state."""
    provider = FakeProvider(plan_steps)
    if toolkit is None:
        toolkit = FakeToolkit()

    set_workflow_context(
        provider=provider,
        model="fake:model",
        toolkit=toolkit,
        emit=lambda s: None,
    )
    state = initial_state(
        task=task,
        max_iterations=max_iterations,
        steps_per_plan=steps_per_plan,
    )
    graph = build_workflow()
    return graph.invoke(state, config={"recursion_limit": recursion_limit})


# ---------------------------------------------------------------------------
# 1. Happy path — single-step plan
# ---------------------------------------------------------------------------

def test_e2e_happy_path_single_step_plan():
    plan_steps = [
        {
            "pattern": "define",
            "intent": "Create root module",
            "parent_id": "root",
            "label": "root_module",
        }
    ]
    final = _run("Add a root module", plan_steps)

    assert final["status"] == "done", f"expected done, got: {final.get('error')}"
    assert final["current_step"] == 1


# ---------------------------------------------------------------------------
# 2. Multi-step plan — later steps reference earlier by label
# ---------------------------------------------------------------------------

def test_e2e_multi_step_plan():
    plan_steps = [
        {
            "pattern": "define",
            "intent": "Define data structure",
            "parent_id": "root",
            "label": "data_struct",
        },
        {
            "pattern": "do",
            "intent": "Process data structure",
            "parent_id": "data_struct",
            "label": "processor",
        },
        {
            "pattern": "test",
            "intent": "Test processor",
            "parent_id": "processor",
        },
    ]
    final = _run("Multi-step task", plan_steps)

    assert final["status"] == "done", f"expected done, got: {final.get('error')}"
    assert final["current_step"] == 3


# ---------------------------------------------------------------------------
# 3. Planner returns invalid JSON → final status=error, error mentions PlanError code
# ---------------------------------------------------------------------------

class BadJsonProvider:
    """Returns unparseable JSON."""

    name: str = "bad_json"

    def complete(self, system: str, user: str, *, model: str) -> str:
        return "THIS IS NOT JSON {{{"

    def complete_with_tools(self, system: str, user: str, *, model: str, tools, tool_choice=None):
        return CompletionResult(text="THIS IS NOT JSON {{{", tool_calls=[])


def test_e2e_planner_returns_invalid_plan():
    toolkit = FakeToolkit()
    set_workflow_context(
        provider=BadJsonProvider(),
        model="fake:model",
        toolkit=toolkit,
        emit=lambda s: None,
    )
    state = initial_state(task="bad plan task")
    graph = build_workflow()
    final = graph.invoke(state, config={"recursion_limit": 50})

    assert final["status"] == "error"
    assert final["error"] is not None
    # PlanError has code AIL-G0144
    assert "AIL-G0144" in final["error"] or "invalid JSON" in final["error"].lower()


# ---------------------------------------------------------------------------
# 4. Coder hits step budget before plan is finished
# ---------------------------------------------------------------------------

def test_e2e_coder_hits_step_budget():
    # Plan has 5 steps but steps_per_plan=2 — coder will hit budget mid-plan.
    plan_steps = [
        {"pattern": "define", "intent": f"Step {i}", "parent_id": "root"}
        for i in range(5)
    ]
    # Use a very large max_iterations so we don't hit that limit first.
    final = _run(
        "big plan task",
        plan_steps,
        steps_per_plan=2,
        max_iterations=200,
        recursion_limit=500,
    )

    assert final["status"] == "error", f"expected error, got: {final.get('status')}"
    assert final["error"] is not None
    assert "G0143" in final["error"]


# ---------------------------------------------------------------------------
# 5. MCP failure during write → final status=error
# ---------------------------------------------------------------------------

def test_e2e_mcp_failure_during_write():
    plan_steps = [
        {
            "pattern": "define",
            "intent": "Write something",
            "parent_id": "root",
        }
    ]
    failing_toolkit = FakeToolkit(fail_on_write=True)
    final = _run("mcp fail task", plan_steps, toolkit=failing_toolkit)

    assert final["status"] == "error"
    assert final["error"] is not None
