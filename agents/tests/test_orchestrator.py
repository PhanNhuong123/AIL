"""23 pytest tests for ail_agent.orchestrator (task 14.1 scaffold)."""

from __future__ import annotations

import json
import logging

import pytest
from langgraph.graph import END, START, StateGraph

from ail_agent.orchestrator import (
    DEFAULT_MAX_ITERATIONS,
    DEFAULT_STEPS_PER_PLAN,
    TERMINAL_STATUSES,
    VALID_STATUSES,
    AILAgentState,
    _coder_node,
    _done_node,
    _enforce_iteration_limit,
    _error_node,
    _planner_node,
    _verify_node,
    build_workflow,
    initial_state,
    route_to_agent,
)
from ail_agent.providers.base import LLMProvider
from ail_agent.registry import get_provider


# ---------------------------------------------------------------------------
# 1. initial_state defaults
# ---------------------------------------------------------------------------

def test_initial_state_defaults() -> None:
    state = initial_state("my task")
    assert state["status"] == "plan"
    assert state["task"] == "my task"
    assert state["plan"] is None
    assert state["current_step"] == 0
    assert state["iteration"] == 0
    assert state["node_id_map"] == {}
    assert state["error"] is None
    assert state["model"] is None
    assert state["mcp_port"] == 7777
    assert state["max_iterations"] == DEFAULT_MAX_ITERATIONS
    assert state["steps_per_plan"] == DEFAULT_STEPS_PER_PLAN
    assert isinstance(state["mcp_port"], int)
    assert isinstance(state["max_iterations"], int)
    assert isinstance(state["steps_per_plan"], int)


# ---------------------------------------------------------------------------
# 2. initial_state custom args
# ---------------------------------------------------------------------------

def test_initial_state_custom_args() -> None:
    state = initial_state(
        "task2",
        model="anthropic:claude-sonnet-4",
        mcp_port=8080,
        max_iterations=10,
        steps_per_plan=5,
    )
    assert state["model"] == "anthropic:claude-sonnet-4"
    assert state["mcp_port"] == 8080
    assert state["max_iterations"] == 10
    assert state["steps_per_plan"] == 5
    assert state["task"] == "task2"


# ---------------------------------------------------------------------------
# 3. JSON round-trip — fully populated state
# ---------------------------------------------------------------------------

def test_state_json_roundtrip_filled() -> None:
    state: AILAgentState = {
        "status": "code",
        "task": "implement feature X",
        "plan": [
            {"id": "s1", "description": "scaffold module", "kind": "function"},
            {"id": "s2", "description": "add tests", "kind": "test"},
            {"id": "s3", "description": "wire CLI", "kind": "command"},
        ],
        "current_step": 1,
        "iteration": 3,
        "node_id_map": {"s1": "node_abc"},
        "error": "boom",
        "model": "anthropic:claude-sonnet-4",
        "mcp_port": 7777,
        "max_iterations": 50,
        "steps_per_plan": 20,
    }
    assert json.loads(json.dumps(state)) == state


# ---------------------------------------------------------------------------
# 4. JSON round-trip — None fields
# ---------------------------------------------------------------------------

def test_state_json_roundtrip_none_fields() -> None:
    state = initial_state("x")
    assert json.loads(json.dumps(state)) == state


# ---------------------------------------------------------------------------
# 5–9. route_to_agent returns valid statuses unchanged
# ---------------------------------------------------------------------------

def test_route_plan() -> None:
    state = initial_state("x")
    state["status"] = "plan"
    assert route_to_agent(state) == "plan"


def test_route_code() -> None:
    state = initial_state("x")
    state["status"] = "code"
    assert route_to_agent(state) == "code"


def test_route_verify() -> None:
    state = initial_state("x")
    state["status"] = "verify"
    assert route_to_agent(state) == "verify"


def test_route_done() -> None:
    state = initial_state("x")
    state["status"] = "done"
    assert route_to_agent(state) == "done"


def test_route_error() -> None:
    state = initial_state("x")
    state["status"] = "error"
    assert route_to_agent(state) == "error"


# ---------------------------------------------------------------------------
# 10. Unknown status returns "error" and mutates state
# ---------------------------------------------------------------------------

def test_route_unknown_status_returns_error() -> None:
    state = initial_state("x")
    state["status"] = "bogus"
    result = route_to_agent(state)
    assert result == "error"
    assert state["status"] == "error"
    assert state["error"] is not None
    assert "Unknown status" in state["error"]


# ---------------------------------------------------------------------------
# 11. None status does not raise
# ---------------------------------------------------------------------------

def test_route_none_status_returns_error() -> None:
    state = initial_state("x")
    state["status"] = None  # type: ignore[assignment]
    result = route_to_agent(state)
    assert result == "error"
    assert state["status"] == "error"


# ---------------------------------------------------------------------------
# 12. Unknown status logs at ERROR level
# ---------------------------------------------------------------------------

def test_route_unknown_status_logs_error(caplog: pytest.LogCaptureFixture) -> None:
    state = initial_state("x")
    state["status"] = "bad_status"
    with caplog.at_level(logging.ERROR, logger="ail_agent.orchestrator"):
        route_to_agent(state)
    assert any("Unknown status" in record.message for record in caplog.records)
    assert any(record.levelno == logging.ERROR for record in caplog.records)


# ---------------------------------------------------------------------------
# 13. _enforce_iteration_limit increments without tripping
# ---------------------------------------------------------------------------

def test_enforce_iteration_limit_increments() -> None:
    state = initial_state("x", max_iterations=5)
    state["iteration"] = 2
    result = _enforce_iteration_limit(state)
    assert result["iteration"] == 3
    assert result["status"] == "plan"
    assert result["error"] is None


# ---------------------------------------------------------------------------
# 14. _enforce_iteration_limit trips at cap
# ---------------------------------------------------------------------------

def test_enforce_iteration_limit_trips() -> None:
    state = initial_state("x", max_iterations=3)
    state["iteration"] = 3  # next increment makes it 4 > 3
    result = _enforce_iteration_limit(state)
    assert result["status"] == "error"
    assert result["error"] is not None
    assert "max_iterations" in result["error"]
    assert "3" in result["error"]


# ---------------------------------------------------------------------------
# 15. build_workflow returns a compiled app with .invoke
# ---------------------------------------------------------------------------

def test_build_workflow_returns_compiled_app() -> None:
    app = build_workflow()
    assert hasattr(app, "invoke"), "compiled LangGraph app must have .invoke"


# ---------------------------------------------------------------------------
# 16. status="done" routes to terminal unchanged
# ---------------------------------------------------------------------------

def test_workflow_terminates_on_done() -> None:
    app = build_workflow()
    state = initial_state("x")
    state["status"] = "done"
    final = app.invoke(state)
    assert final["status"] == "done"
    assert final["error"] is None


# ---------------------------------------------------------------------------
# 17. status="error" routes to terminal, error field preserved
# ---------------------------------------------------------------------------

def test_workflow_terminates_on_error() -> None:
    app = build_workflow()
    state = initial_state("x")
    state["status"] = "error"
    state["error"] = "pre-existing error"
    final = app.invoke(state)
    assert final["status"] == "error"
    assert final["error"] == "pre-existing error"


# ---------------------------------------------------------------------------
# 18. End-to-end with mocked nodes: plan → code → verify → done
# ---------------------------------------------------------------------------

def test_workflow_e2e_with_mocked_nodes() -> None:
    """Build a parallel StateGraph using route_to_agent with mock handlers."""

    def plan_mock(state: AILAgentState) -> AILAgentState:
        state = _enforce_iteration_limit(state)
        if state.get("status") == "error":
            return state
        state["status"] = "code"
        return state

    def code_mock(state: AILAgentState) -> AILAgentState:
        state = _enforce_iteration_limit(state)
        if state.get("status") == "error":
            return state
        state["status"] = "verify"
        return state

    def verify_mock(state: AILAgentState) -> AILAgentState:
        state = _enforce_iteration_limit(state)
        if state.get("status") == "error":
            return state
        state["status"] = "done"
        return state

    routing_map = {s: s for s in VALID_STATUSES}

    g: StateGraph = StateGraph(AILAgentState)
    g.add_node("plan", plan_mock)
    g.add_node("code", code_mock)
    g.add_node("verify", verify_mock)
    g.add_node("done", _done_node)
    g.add_node("error", _error_node)

    g.add_conditional_edges(START, route_to_agent, routing_map)
    g.add_conditional_edges("plan", route_to_agent, routing_map)
    g.add_conditional_edges("code", route_to_agent, routing_map)
    g.add_conditional_edges("verify", route_to_agent, routing_map)
    g.add_edge("done", END)
    g.add_edge("error", END)

    app = g.compile()
    final = app.invoke(initial_state("demo"))

    assert final["status"] == "done"
    assert final["error"] is None
    assert final["iteration"] == 3


# ---------------------------------------------------------------------------
# 19. Iteration guard prevents infinite loop
# ---------------------------------------------------------------------------

def test_workflow_max_iterations_prevents_infinite_loop() -> None:
    """A coder that loops is stopped by the iteration guard."""

    def plan_mock(state: AILAgentState) -> AILAgentState:
        state = _enforce_iteration_limit(state)
        if state.get("status") == "error":
            return state
        state["status"] = "code"
        return state

    def code_mock(state: AILAgentState) -> AILAgentState:
        # Deliberately leave status as "code" to loop back
        state = _enforce_iteration_limit(state)
        if state.get("status") == "error":
            return state
        # Do NOT advance status — loops back to "code"
        return state

    routing_map = {s: s for s in VALID_STATUSES}

    g: StateGraph = StateGraph(AILAgentState)
    g.add_node("plan", plan_mock)
    g.add_node("code", code_mock)
    g.add_node("verify", _verify_node)
    g.add_node("done", _done_node)
    g.add_node("error", _error_node)

    g.add_conditional_edges(START, route_to_agent, routing_map)
    g.add_conditional_edges("plan", route_to_agent, routing_map)
    g.add_conditional_edges("code", route_to_agent, routing_map)
    g.add_conditional_edges("verify", route_to_agent, routing_map)
    g.add_edge("done", END)
    g.add_edge("error", END)

    app = g.compile()
    start = initial_state("demo", max_iterations=2)
    # Use a generous LangGraph recursion_limit so our iteration guard trips first
    final = app.invoke(start, config={"recursion_limit": 50})

    assert final["status"] == "error"
    assert final["error"] is not None
    assert "max_iterations (2) exceeded" in final["error"]


# ---------------------------------------------------------------------------
# 20. VALID_STATUSES matches the expected routing strings
# ---------------------------------------------------------------------------

def test_valid_statuses_matches_routing_strings() -> None:
    assert VALID_STATUSES == {"plan", "code", "verify", "done", "error"}


# ---------------------------------------------------------------------------
# 21. Stub nodes raise NotImplementedError mentioning 14.3
# ---------------------------------------------------------------------------

def test_orchestrator_stub_nodes_raise_not_implemented() -> None:
    for stub in (_planner_node, _coder_node, _verify_node):
        fresh = initial_state("x")
        with pytest.raises(NotImplementedError) as exc_info:
            stub(fresh)
        assert "14.3" in str(exc_info.value), (
            f"{stub.__name__} must mention task 14.3 in NotImplementedError"
        )


# ---------------------------------------------------------------------------
# 22. LLMProvider is importable and runtime-checkable
# ---------------------------------------------------------------------------

def test_provider_protocol_importable() -> None:
    assert LLMProvider is not None
    # runtime_checkable means isinstance checks work
    # A class with the right shape should pass; an int should not
    class FakeProvider:
        name = "fake"

        def complete(self, system: str, user: str, *, model: str) -> str:
            return ""

    assert isinstance(FakeProvider(), LLMProvider)
    assert not isinstance(42, LLMProvider)


# ---------------------------------------------------------------------------
# 23. get_provider raises NotImplementedError mentioning 14.2
# ---------------------------------------------------------------------------

def test_registry_get_provider_raises_not_implemented() -> None:
    with pytest.raises(NotImplementedError) as exc_info:
        get_provider("anthropic:claude-sonnet-4")
    assert "14.2" in str(exc_info.value)
