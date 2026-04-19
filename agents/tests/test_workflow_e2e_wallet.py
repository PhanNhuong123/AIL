"""End-to-end workflow tests scoped to the wallet_service domain (Phase 15 task 15.2).

These tests exercise the full build_workflow() LangGraph graph with
FakeProvider and FakeToolkit substituted for real LLM and MCP backends.
They prove four acceptance criteria specific to wallet error-handling plans:

  1. A 3-step domain-error plan reaches status="done" and populates node_id_map.
  2. The workflow respects a max_iterations budget without tripping G0142.
  3. The MCP call sequence matches the documented contract (status → N writes → status).
  4. VERIFY_OK_LINE is emitted through a custom emit callable.
"""
from __future__ import annotations

from ail_agent.orchestrator import (
    AILAgentState,
    build_workflow,
    clear_workflow_context,
    initial_state,
    set_workflow_context,
)
from ail_agent.progress import VERIFY_OK_LINE

# Import shared fakes and helpers from the existing mocked-workflow test module.
# The try/except handles both installed-package and relative-import scenarios.
try:
    from agents.tests.test_workflow_e2e_mocked import (
        FakeProvider,
        FakeToolkit,
        _ROOT_ID,
        _run,
    )
except ImportError:
    from .test_workflow_e2e_mocked import (  # type: ignore[no-redef]
        FakeProvider,
        FakeToolkit,
        _ROOT_ID,
        _run,
    )

# ---------------------------------------------------------------------------
# Wallet-specific plan steps (used across all four tests)
# ---------------------------------------------------------------------------

_WALLET_PLAN_STEPS = [
    {
        "pattern": "do",
        "intent": "Handle insufficient balance by raising a domain error",
        "parent_id": "root",
        "label": "handle_insufficient_balance",
    },
    {
        "pattern": "do",
        "intent": "Handle invalid user by raising a domain error",
        "parent_id": "handle_insufficient_balance",
        "label": "handle_invalid_user",
    },
    {
        "pattern": "do",
        "intent": "Log transfer error to observability sink",
        "parent_id": "handle_invalid_user",
        "label": "log_transfer_error",
    },
]

_WALLET_TASK = "Add error handling to transfer_money in wallet_service"

# ---------------------------------------------------------------------------
# Test 1: 3-step plan reaches done and populates node_id_map
# ---------------------------------------------------------------------------


def test_wallet_agent_plan_creates_three_nodes():
    """Proves a 3-step wallet error-handling plan drives the agent to done with all labels in node_id_map."""
    final = _run(_WALLET_TASK, _WALLET_PLAN_STEPS)

    assert final["status"] == "done", (
        f"expected status='done', got {final['status']!r}; error={final.get('error')!r}"
    )
    assert final["current_step"] == 3, (
        f"expected current_step=3, got {final['current_step']}"
    )

    node_id_map = final["node_id_map"]
    assert len(node_id_map) >= 4, (
        f"expected at least 4 entries in node_id_map (root + 3 labels), got {len(node_id_map)}: {list(node_id_map)}"
    )
    assert "handle_insufficient_balance" in node_id_map, (
        f"'handle_insufficient_balance' missing from node_id_map; keys={list(node_id_map)}"
    )
    assert "handle_invalid_user" in node_id_map, (
        f"'handle_invalid_user' missing from node_id_map; keys={list(node_id_map)}"
    )
    assert "log_transfer_error" in node_id_map, (
        f"'log_transfer_error' missing from node_id_map; keys={list(node_id_map)}"
    )

    iteration = final.get("iteration", 0)
    max_iterations = final["max_iterations"]
    assert iteration <= max_iterations, (
        f"iteration {iteration} exceeded max_iterations {max_iterations}"
    )


# ---------------------------------------------------------------------------
# Test 2: workflow respects max_iterations budget
# ---------------------------------------------------------------------------


def test_wallet_agent_respects_max_iterations():
    """Proves the workflow completes the wallet plan within a max_iterations=20 budget without tripping G0142."""
    final = _run(_WALLET_TASK, _WALLET_PLAN_STEPS, max_iterations=20)

    assert final["status"] == "done", (
        f"expected status='done', got {final['status']!r}; error={final.get('error')!r}"
    )

    iteration = final.get("iteration", 0)
    assert iteration <= 20, (
        f"iteration {iteration} exceeded budget of 20"
    )

    error = final.get("error") or ""
    assert "G0142" not in error, (
        f"iteration-limit error code G0142 found unexpectedly: {error!r}"
    )


# ---------------------------------------------------------------------------
# Test 3: MCP call sequence matches the documented contract
# ---------------------------------------------------------------------------


def test_wallet_agent_call_sequence_matches_contract():
    """Proves the MCP call sequence is: status (root) → write × 3 → status (verify)."""
    toolkit = FakeToolkit()
    _run(_WALLET_TASK, _WALLET_PLAN_STEPS, toolkit=toolkit)

    calls = toolkit.calls
    assert len(calls) == 5, (
        f"expected exactly 5 MCP calls (1 status + 3 writes + 1 status), got {len(calls)}: {calls}"
    )

    # First call: root resolution via ail.status
    assert calls[0][0] == "ail.status", (
        f"expected calls[0] to be 'ail.status', got {calls[0][0]!r}"
    )

    # Calls 2, 3, 4 (indices 1–3): ail.write for each plan step
    for i in range(1, 4):
        assert calls[i][0] == "ail.write", (
            f"expected calls[{i}] to be 'ail.write', got {calls[i][0]!r}"
        )

    # Last call: verify-stage status check
    assert calls[4][0] == "ail.status", (
        f"expected calls[4] to be 'ail.status', got {calls[4][0]!r}"
    )

    # Validate each write call's arguments against the plan steps
    for i, step in enumerate(_WALLET_PLAN_STEPS):
        call_args = calls[i + 1][1]
        assert call_args["pattern"] == "do", (
            f"calls[{i + 1}]['pattern'] expected 'do', got {call_args['pattern']!r}"
        )
        assert call_args["intent"] == step["intent"], (
            f"calls[{i + 1}]['intent'] mismatch: expected {step['intent']!r}, got {call_args['intent']!r}"
        )

    # First write: parent_id must be the resolved root UUID
    first_write_args = calls[1][1]
    assert first_write_args["parent_id"] == _ROOT_ID, (
        f"first write parent_id expected {_ROOT_ID!r}, got {first_write_args['parent_id']!r}"
    )

    # Second write: parent_id must be the node_id returned by the first write
    expected_node_id_1 = f"bbbbbbbb-0000-0000-0000-{1:012d}"
    second_write_args = calls[2][1]
    assert second_write_args["parent_id"] == expected_node_id_1, (
        f"second write parent_id expected {expected_node_id_1!r}, got {second_write_args['parent_id']!r}"
    )

    # Third write: parent_id must be the node_id returned by the second write
    expected_node_id_2 = f"bbbbbbbb-0000-0000-0000-{2:012d}"
    third_write_args = calls[3][1]
    assert third_write_args["parent_id"] == expected_node_id_2, (
        f"third write parent_id expected {expected_node_id_2!r}, got {third_write_args['parent_id']!r}"
    )


# ---------------------------------------------------------------------------
# Test 4: VERIFY_OK_LINE is emitted through a custom emit callable
# ---------------------------------------------------------------------------


def test_wallet_agent_verify_ok_line_emitted():
    """Proves that VERIFY_OK_LINE is delivered to the emit callable after successful verification."""
    emitted: list[str] = []

    def record(line: str) -> None:
        emitted.append(line)

    provider = FakeProvider(_WALLET_PLAN_STEPS)
    toolkit = FakeToolkit()

    # Explicit setup guard: clears any context left by a prior test, and the
    # try/finally ensures teardown even if set_workflow_context partially fails.
    clear_workflow_context()
    try:
        set_workflow_context(
            provider=provider,
            model="fake:model",
            toolkit=toolkit,
            emit=record,
        )
        state = initial_state(task=_WALLET_TASK, max_iterations=20, steps_per_plan=20)
        graph = build_workflow()
        final = graph.invoke(state, config={"recursion_limit": 200})
    finally:
        clear_workflow_context()

    assert final["status"] == "done", (
        f"expected status='done', got {final['status']!r}; error={final.get('error')!r}"
    )
    assert VERIFY_OK_LINE in emitted, (
        f"VERIFY_OK_LINE not found in emitted lines; emitted={emitted!r}"
    )
