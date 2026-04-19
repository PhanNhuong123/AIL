"""Coder stub for the AIL agent loop — real body lands in task 14.3."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ail_agent.orchestrator import AILAgentState


def run_coder(state: "AILAgentState") -> "AILAgentState":
    """Execute the current plan step via the MCP server.

    Called by ``_coder_node`` in orchestrator.  Responsibilities (task 14.3):
    - Read ``state["plan"][state["current_step"]]`` to get the active step.
    - Call the appropriate MCP tool and record returned node IDs in
      ``state["node_id_map"]``.
    - Advance ``state["current_step"]`` by 1 after success.

    Two distinct completion checks that task 14.3 MUST implement:
    1. ``current_step >= len(plan)`` — all steps done; set status to ``"verify"``.
    2. ``current_step >= steps_per_plan`` — step budget exhausted before the plan
       finished; set status to ``"error"`` with an appropriate message
       (AIL-G0143).

    These are two separate conditions with different semantics and must NOT be
    collapsed into a single branch.
    """
    raise NotImplementedError("run_coder: task 14.3")
