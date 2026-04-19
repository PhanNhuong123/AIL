"""pytest tests for ail_agent.orchestrator (tasks 14.1 + 14.3 wiring)."""

from __future__ import annotations

import json
import logging
from unittest.mock import Mock, patch

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
    clear_workflow_context,
    get_workflow_context,
    initial_state,
    route_to_agent,
    set_workflow_context,
)
from ail_agent.providers.base import LLMProvider


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
# 21. LLMProvider is importable and runtime-checkable
# ---------------------------------------------------------------------------

def test_provider_protocol_importable() -> None:
    assert LLMProvider is not None
    # runtime_checkable means isinstance checks work
    # A class with the right shape should pass; an int should not
    class FakeProvider:
        name = "fake"

        def complete(self, system: str, user: str, *, model: str) -> str:
            return ""

        def complete_with_tools(self, system: str, user: str, *, model: str, tools, tool_choice=None):  # type: ignore[override]
            return {"text": None, "tool_calls": []}

    assert isinstance(FakeProvider(), LLMProvider)
    assert not isinstance(42, LLMProvider)


# ---------------------------------------------------------------------------
# 22–27. Workflow context and node delegation (task 14.3 wiring)
# ---------------------------------------------------------------------------

@pytest.fixture(autouse=True)
def _reset_workflow_context():
    """Ensure workflow context is clean before and after every test."""
    clear_workflow_context()
    yield
    clear_workflow_context()


def _make_state(**overrides) -> AILAgentState:
    """Return an initial state with optional field overrides."""
    state = initial_state("test task")
    state.update(overrides)  # type: ignore[typeddict-item]
    return state


class TestWorkflowContext:
    """Tests for set_workflow_context / get_workflow_context / clear_workflow_context."""

    def test_set_workflow_context_persists_across_calls(self) -> None:
        """Successive set calls merge into the context without overwriting earlier keys."""
        p1 = Mock()
        set_workflow_context(provider=p1)
        set_workflow_context(model="m1")
        ctx = get_workflow_context()
        assert ctx["provider"] is p1
        assert ctx["model"] == "m1"
        clear_workflow_context()
        assert get_workflow_context() == {}

    def test_set_workflow_context_none_values_ignored(self) -> None:
        """Passing None for a key must not overwrite an existing value."""
        p1 = Mock()
        set_workflow_context(provider=p1)
        set_workflow_context(provider=None)  # should be a no-op
        assert get_workflow_context()["provider"] is p1

    def test_get_workflow_context_returns_shallow_copy(self) -> None:
        """Mutating the returned dict must not affect the module-level context."""
        set_workflow_context(model="v1")
        ctx = get_workflow_context()
        ctx["model"] = "mutated"
        assert get_workflow_context()["model"] == "v1"


class TestPlannerNodeDelegation:
    """Tests for _planner_node delegation and error paths."""

    def test_planner_node_delegates_to_run_planner(self) -> None:
        """_planner_node must call run_planner with the injected provider and model."""
        provider = Mock()
        sentinel_state = _make_state(status="code")

        with patch("ail_agent.planner.run_planner", return_value=sentinel_state) as mock_planner:
            set_workflow_context(provider=provider, model="test-model")
            state = _make_state()
            result = _planner_node(state)

        mock_planner.assert_called_once()
        call_args = mock_planner.call_args
        # First positional arg is the state (with iteration already incremented by guard).
        assert call_args.kwargs["provider"] is provider
        assert call_args.kwargs["model"] == "test-model"
        assert result is sentinel_state

    def test_planner_node_errors_when_provider_missing(self) -> None:
        """_planner_node must return an error state when provider is absent from context."""
        set_workflow_context(model="some-model")  # provider intentionally omitted
        state = _make_state()
        result = _planner_node(state)
        assert result["status"] == "error"
        assert result["error"] is not None
        assert "provider" in result["error"]

    def test_planner_node_errors_when_model_missing(self) -> None:
        """_planner_node must return an error state when model is absent from context."""
        set_workflow_context(provider=Mock())  # model intentionally omitted
        state = _make_state()
        result = _planner_node(state)
        assert result["status"] == "error"
        assert result["error"] is not None

    def test_planner_node_skips_delegation_when_iteration_limit_hit(self) -> None:
        """After the iteration guard trips, run_planner must NOT be called."""
        set_workflow_context(provider=Mock(), model="x")
        state = _make_state(max_iterations=0, iteration=0)  # guard will trip on first call

        with patch("ail_agent.planner.run_planner") as mock_planner:
            result = _planner_node(state)

        mock_planner.assert_not_called()
        assert result["status"] == "error"
        assert "max_iterations" in result["error"]


class TestCoderNodeDelegation:
    """Tests for _coder_node delegation and error paths."""

    def test_coder_node_delegates_to_run_coder(self) -> None:
        """_coder_node must call run_coder with the injected toolkit."""
        toolkit = Mock()
        sentinel_state = _make_state(status="code")

        with patch("ail_agent.coder.run_coder", return_value=sentinel_state) as mock_coder:
            set_workflow_context(toolkit=toolkit)
            state = _make_state()
            result = _coder_node(state)

        mock_coder.assert_called_once()
        call_args = mock_coder.call_args
        assert call_args.kwargs["toolkit"] is toolkit
        assert result is sentinel_state

    def test_coder_node_errors_when_toolkit_missing(self) -> None:
        """_coder_node must return an error state when toolkit is absent from context."""
        # context is empty (autouse fixture clears it)
        state = _make_state()
        result = _coder_node(state)
        assert result["status"] == "error"
        assert result["error"] is not None
        assert "toolkit" in result["error"]

    def test_coder_node_skips_delegation_when_iteration_limit_hit(self) -> None:
        """After the iteration guard trips, run_coder must NOT be called."""
        set_workflow_context(toolkit=Mock())
        state = _make_state(max_iterations=0, iteration=0)

        with patch("ail_agent.coder.run_coder") as mock_coder:
            result = _coder_node(state)

        mock_coder.assert_not_called()
        assert result["status"] == "error"


class TestVerifyNodeDelegation:
    """Tests for _verify_node delegation and error paths."""

    def test_verify_node_delegates_to_run_verify(self) -> None:
        """_verify_node must call run_verify with toolkit and emit from context."""
        toolkit = Mock()
        emit_fn = Mock()
        sentinel_state = _make_state(status="done")

        with patch("ail_agent.verify.run_verify", return_value=sentinel_state) as mock_verify:
            set_workflow_context(toolkit=toolkit, emit=emit_fn)
            state = _make_state()
            result = _verify_node(state)

        mock_verify.assert_called_once()
        call_args = mock_verify.call_args
        assert call_args.kwargs["toolkit"] is toolkit
        assert call_args.kwargs["emit"] is emit_fn
        assert result is sentinel_state

    def test_verify_node_passes_none_emit_when_not_set(self) -> None:
        """_verify_node must pass emit=None when the context has no emit callable."""
        toolkit = Mock()
        sentinel_state = _make_state(status="done")

        with patch("ail_agent.verify.run_verify", return_value=sentinel_state) as mock_verify:
            set_workflow_context(toolkit=toolkit)  # emit not set
            state = _make_state()
            _verify_node(state)

        call_args = mock_verify.call_args
        assert call_args.kwargs.get("emit") is None

    def test_verify_node_errors_when_toolkit_missing(self) -> None:
        """_verify_node must return an error state when toolkit is absent from context."""
        state = _make_state()
        result = _verify_node(state)
        assert result["status"] == "error"
        assert result["error"] is not None
        assert "toolkit" in result["error"]

    def test_verify_node_skips_delegation_when_iteration_limit_hit(self) -> None:
        """After the iteration guard trips, run_verify must NOT be called."""
        set_workflow_context(toolkit=Mock())
        state = _make_state(max_iterations=0, iteration=0)

        with patch("ail_agent.verify.run_verify") as mock_verify:
            result = _verify_node(state)

        mock_verify.assert_not_called()
        assert result["status"] == "error"
