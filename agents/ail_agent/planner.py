"""Planner worker: calls an LLM, parses the response, returns a state delta."""
from __future__ import annotations

import logging
from typing import Any, cast

from ail_agent.errors import AgentError, PlanError, ProviderError
from ail_agent.orchestrator import AILAgentState
from ail_agent.plan_format import parse_plan
from ail_agent.prompts import PLANNER_SYSTEM_PROMPT, PLANNER_USER_TEMPLATE
from ail_agent.providers.base import LLMProvider

logger = logging.getLogger(__name__)


def run_planner(
    state: AILAgentState,
    *,
    provider: LLMProvider,
    model: str,
) -> AILAgentState:
    """Call the LLM with the planner prompt and update state.

    On success: ``state["plan"]`` is a list of PlanStep dicts and
    ``state["status"] = "code"``.
    On failure: ``state["status"] = "error"`` and ``state["error"]`` is the
    formatted error string.
    The function never raises; failures are reflected in the state.
    """
    new_state: AILAgentState = cast(AILAgentState, {**state})

    # --- 1. Read required task field ---
    task: str = new_state["task"]

    # --- 2. Read optional project_context ---
    raw_context: Any = new_state.get("project_context", "(none)")  # type: ignore[typeddict-item]
    project_context_str: str = str(raw_context) if raw_context is not None else "(none)"

    # --- 3. Build prompt messages ---
    system: str = PLANNER_SYSTEM_PROMPT
    user: str = PLANNER_USER_TEMPLATE.format(task=task, context=project_context_str)

    # --- 4. Call the provider ---
    try:
        response_text: str = provider.complete(system, user, model=model)
    except AgentError as exc:
        logger.warning("Planner provider call failed: %s", exc)
        new_state["status"] = "error"
        new_state["error"] = str(exc)
        return new_state

    # --- 5. Parse the response ---
    try:
        plan_steps = parse_plan(response_text)
    except PlanError as exc:
        logger.warning("Planner response parse failed: %s", exc)
        new_state["status"] = "error"
        new_state["error"] = str(exc)
        return new_state

    # --- 6. Success path ---
    new_state["plan"] = cast(list[dict], plan_steps)
    new_state["current_step"] = 0
    new_state["status"] = "code"
    new_state["error"] = None
    return new_state
