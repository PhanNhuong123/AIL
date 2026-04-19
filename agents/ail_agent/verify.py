"""Verify stub for the AIL agent loop — real body lands in task 14.3."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ail_agent.orchestrator import AILAgentState


def run_verify(state: "AILAgentState") -> "AILAgentState":
    """Verify the generated output meets all AIL contracts.

    Called by ``_verify_node`` in orchestrator.  Responsibilities (task 14.3):
    - Invoke the MCP ``ail.verify`` tool on the built graph.
    - On success, set ``state["status"] = "done"``.
    - On contract violations, set ``state["status"] = "plan"`` to re-plan, or
      ``"error"`` if the iteration limit was already reached.
    """
    raise NotImplementedError("run_verify: task 14.3")
