"""AILAgentState, routing, iteration guard, and LangGraph workflow scaffold."""

from __future__ import annotations

import json
import logging
from typing import Optional

from langgraph.graph import END, START, StateGraph
from typing_extensions import TypedDict

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

VALID_STATUSES: frozenset[str] = frozenset({"plan", "code", "verify", "done", "error"})
TERMINAL_STATUSES: frozenset[str] = frozenset({"done", "error"})
DEFAULT_MAX_ITERATIONS: int = 50
DEFAULT_STEPS_PER_PLAN: int = 20


# ---------------------------------------------------------------------------
# State
# ---------------------------------------------------------------------------

class AILAgentState(TypedDict):
    """JSON-serializable LangGraph state for the AIL agent loop.

    All fields are plain Python scalars or JSON-safe containers so the state
    can survive ``json.dumps`` round-trips at any point in the workflow.
    """

    status: str
    task: str
    plan: Optional[list[dict]]
    current_step: int
    iteration: int
    node_id_map: dict  # dict[str, str] at runtime
    error: Optional[str]
    model: Optional[str]
    mcp_port: int
    max_iterations: int
    steps_per_plan: int


# ---------------------------------------------------------------------------
# initial_state
# ---------------------------------------------------------------------------

def initial_state(
    task: str,
    model: Optional[str] = None,
    mcp_port: int = 7777,
    max_iterations: int = DEFAULT_MAX_ITERATIONS,
    steps_per_plan: int = DEFAULT_STEPS_PER_PLAN,
) -> AILAgentState:
    """Return a freshly-initialised ``AILAgentState`` dict.

    Ends with ``assert json.dumps(state) is not None`` so misuse (e.g. a
    non-serialisable value sneaking in) fails loudly during development and
    tests rather than silently at serialisation time.
    """
    state: AILAgentState = {
        "status": "plan",
        "task": task,
        "plan": None,
        "current_step": 0,
        "iteration": 0,
        "node_id_map": {},
        "error": None,
        "model": model,
        "mcp_port": mcp_port,
        "max_iterations": max_iterations,
        "steps_per_plan": steps_per_plan,
    }
    assert json.dumps(state) is not None  # noqa: S101 — intentional dev guard
    return state


# ---------------------------------------------------------------------------
# Routing
# ---------------------------------------------------------------------------

def route_to_agent(state: AILAgentState) -> str:
    """Return the name of the next node to visit.

    For every known status the return value equals the status string so node
    names and routing strings stay in sync via a single ``VALID_STATUSES``
    source-of-truth.  Unknown or ``None`` statuses are handled gracefully:
    the error is logged, the state is mutated to ``"error"``, and ``"error"``
    is returned — no exception is raised (issue 14.1-D).
    """
    status = state.get("status")
    if status in VALID_STATUSES:
        return status  # type: ignore[return-value]
    msg = f"Unknown status: {status!r}. Routing to error."
    logger.error(msg)
    state["error"] = msg
    state["status"] = "error"
    return "error"


# ---------------------------------------------------------------------------
# Iteration guard
# ---------------------------------------------------------------------------

def _enforce_iteration_limit(state: AILAgentState) -> AILAgentState:
    """Increment ``iteration`` and trip to error if the cap is exceeded.

    Returns the (possibly mutated) state dict.  Always returns — never raises.
    """
    state["iteration"] = state.get("iteration", 0) + 1
    max_iter: int = state.get("max_iterations", DEFAULT_MAX_ITERATIONS)
    if state["iteration"] > max_iter:
        state["status"] = "error"
        state["error"] = f"max_iterations ({max_iter}) exceeded"
    return state


# ---------------------------------------------------------------------------
# Node stubs
# ---------------------------------------------------------------------------

def _planner_node(state: AILAgentState) -> AILAgentState:
    """Stub planner node — real body lands in task 14.3."""
    state = _enforce_iteration_limit(state)
    if state.get("status") == "error":
        return state
    raise NotImplementedError("_planner_node: task 14.3")


def _coder_node(state: AILAgentState) -> AILAgentState:
    """Stub coder node — real body lands in task 14.3."""
    state = _enforce_iteration_limit(state)
    if state.get("status") == "error":
        return state
    raise NotImplementedError("_coder_node: task 14.3")


def _verify_node(state: AILAgentState) -> AILAgentState:
    """Stub verify node — real body lands in task 14.3."""
    state = _enforce_iteration_limit(state)
    if state.get("status") == "error":
        return state
    raise NotImplementedError("_verify_node: task 14.3")


def _done_node(state: AILAgentState) -> AILAgentState:
    """Terminal done node — returns state unchanged."""
    return state


def _error_node(state: AILAgentState) -> AILAgentState:
    """Terminal error node — returns state unchanged."""
    return state


# ---------------------------------------------------------------------------
# Workflow factory
# ---------------------------------------------------------------------------

def build_workflow():
    """Compile and return the LangGraph ``CompiledStateGraph``.

    Node names are **exactly** the strings in ``VALID_STATUSES`` so that
    ``route_to_agent`` can return a status and LangGraph routes directly to
    the matching node — no mapping table needed.
    """
    g: StateGraph = StateGraph(AILAgentState)

    g.add_node("plan", _planner_node)
    g.add_node("code", _coder_node)
    g.add_node("verify", _verify_node)
    g.add_node("done", _done_node)
    g.add_node("error", _error_node)

    routing_map = {s: s for s in VALID_STATUSES}

    g.add_conditional_edges(START, route_to_agent, routing_map)
    g.add_conditional_edges("plan", route_to_agent, routing_map)
    g.add_conditional_edges("code", route_to_agent, routing_map)
    g.add_conditional_edges("verify", route_to_agent, routing_map)

    g.add_edge("done", END)
    g.add_edge("error", END)

    return g.compile()
