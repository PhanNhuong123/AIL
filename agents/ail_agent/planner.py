"""Planner stub for the AIL agent loop — real body lands in task 14.3."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ail_agent.orchestrator import AILAgentState


def run_planner(state: "AILAgentState") -> "AILAgentState":
    """Decompose the task into an ordered plan of steps.

    Called by ``_planner_node`` in orchestrator.  Responsibilities (task 14.3):
    - Send the task description to the LLM via the registered provider.
    - Parse the LLM response into ``list[dict]`` and assign to ``state["plan"]``.
    - Set ``state["status"] = "code"`` to advance the loop.
    - Populate ``state["current_step"] = 0``.
    """
    raise NotImplementedError("run_planner: task 14.3")
