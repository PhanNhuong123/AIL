"""Coder worker: applies one PlanStep per invocation via MCP ail.write."""

from __future__ import annotations

import re
import uuid
from typing import Any

from ail_agent.errors import AgentError, MCPConnectionError, StepBudgetError
from ail_agent.mcp_toolkit import MCPToolkit
from ail_agent.orchestrator import AILAgentState
from ail_agent.plan_format import PlanStep

# Regex for a canonical UUID v4 (or any UUID-format string).
_UUID_RE = re.compile(
    r"^[0-9a-f]{8}-([0-9a-f]{4}-){3}[0-9a-f]{12}$",
    re.IGNORECASE,
)

_NO_ROOT_ERROR = "[AIL-G0140] project has no root node — run ail init first"


def _is_uuid(value: str) -> bool:
    """Return True if *value* looks like a UUID string."""
    return bool(_UUID_RE.match(value))


def run_coder(
    state: AILAgentState,
    *,
    toolkit: MCPToolkit,
) -> AILAgentState:
    """Apply ONE plan step (the one at index `current_step`) via MCP ail.write.

    State transitions:
      - plan finished (current_step >= len(plan))    → status="verify"
      - step budget hit (current_step >= steps_per_plan) → status="error"
      - successful write                              → current_step += 1, status="code"
      - parent_id resolution failure                  → status="error"
      - MCP error                                     → status="error"
    """
    plan: list[dict] = state["plan"] or []
    current_step: int = state["current_step"]
    steps_per_plan: int = state["steps_per_plan"]

    # ------------------------------------------------------------------
    # 1a. Plan complete — takes precedence over budget check.
    # ------------------------------------------------------------------
    if current_step >= len(plan):
        new_state = dict(state)
        new_state["status"] = "verify"
        return new_state  # type: ignore[return-value]

    # ------------------------------------------------------------------
    # 1b. Step budget exceeded mid-plan.
    # ------------------------------------------------------------------
    if current_step >= steps_per_plan:
        new_state = dict(state)
        new_state["status"] = "error"
        new_state["error"] = (
            f"[AIL-G0143] steps_per_plan ({steps_per_plan}) exceeded before plan finished"
        )
        return new_state  # type: ignore[return-value]

    # ------------------------------------------------------------------
    # 2. Resolve "root" once and cache in node_id_map.
    # ------------------------------------------------------------------
    # Shallow-copy state; deep-copy only node_id_map because we may update it.
    new_state = dict(state)
    new_state["node_id_map"] = dict(state["node_id_map"])
    node_id_map: dict[str, str] = new_state["node_id_map"]

    if "root" not in node_id_map:
        try:
            status_resp: dict[str, Any] = toolkit.call("ail.status", {})
        except (MCPConnectionError, AgentError) as exc:
            new_state["status"] = "error"
            new_state["error"] = str(exc)
            return new_state  # type: ignore[return-value]

        root_id: str | None = status_resp.get("root_id")
        if not root_id:
            new_state["status"] = "error"
            new_state["error"] = _NO_ROOT_ERROR
            return new_state  # type: ignore[return-value]

        node_id_map["root"] = root_id

    # ------------------------------------------------------------------
    # 3. Pick the current step.
    # ------------------------------------------------------------------
    step: dict = plan[current_step]

    # ------------------------------------------------------------------
    # 4. Resolve parent_id.
    # ------------------------------------------------------------------
    raw_parent: str = step["parent_id"]

    if raw_parent == "root":
        resolved_parent = node_id_map["root"]
    elif _is_uuid(raw_parent):
        resolved_parent = raw_parent
    else:
        # Label reference — must already be in node_id_map.
        if raw_parent not in node_id_map:
            new_state["status"] = "error"
            new_state["error"] = (
                f"[AIL-G0140] step {current_step}: unknown parent_id label {raw_parent!r};"
                " not in node_id_map and not a UUID"
            )
            return new_state  # type: ignore[return-value]
        resolved_parent = node_id_map[raw_parent]

    # ------------------------------------------------------------------
    # 5. Build ail.write arguments.
    # ------------------------------------------------------------------
    args: dict[str, Any] = {
        "parent_id": resolved_parent,
        "pattern": step["pattern"],
        "intent": step["intent"],
    }
    if "expression" in step:
        args["expression"] = step["expression"]
    if "contracts" in step:
        args["contracts"] = step["contracts"]
    if "metadata" in step:
        args["metadata"] = step["metadata"]

    # ------------------------------------------------------------------
    # 6. Call ail.write.
    # ------------------------------------------------------------------
    try:
        response: dict[str, Any] = toolkit.call("ail.write", args)
    except (MCPConnectionError, AgentError) as exc:
        new_state["status"] = "error"
        new_state["error"] = str(exc)
        return new_state  # type: ignore[return-value]

    # ------------------------------------------------------------------
    # 7. Record the new node.
    # ------------------------------------------------------------------
    try:
        new_node_id = response["node_id"]
        if not isinstance(new_node_id, str) or not new_node_id:
            raise KeyError("node_id missing or empty")
    except (KeyError, TypeError) as exc:
        new_state["status"] = "error"
        new_state["error"] = f"[AIL-G0140] ail.write returned no node_id: {exc}"
        return new_state  # type: ignore[return-value]
    label_key: str = step.get("label") or step["intent"]
    node_id_map[label_key] = new_node_id

    # ------------------------------------------------------------------
    # 8. Advance.
    # ------------------------------------------------------------------
    new_state["current_step"] = current_step + 1
    new_state["status"] = "code"
    new_state["error"] = ""

    return new_state  # type: ignore[return-value]
