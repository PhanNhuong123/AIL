"""Verify worker: lightweight sanity check via ail.status."""

from __future__ import annotations

from typing import Callable

from ail_agent.errors import AgentError, MCPConnectionError
from ail_agent.mcp_toolkit import MCPToolkit
from ail_agent.orchestrator import AILAgentState
from ail_agent.progress import VERIFY_OK_LINE


def run_verify(
    state: AILAgentState,
    *,
    toolkit: MCPToolkit,
    emit: Callable[[str], None] | None = None,
) -> AILAgentState:
    """Run a basic post-coding sanity check.

    Calls ail.status to confirm the project still loads and has at least one node.
    Emits VERIFY_OK_LINE via the injected `emit` callable (default: no-op).
    Sets status="done" on success, status="error" on failure.

    Does NOT call MCP ail.verify (that is expensive). The user is told via the
    emitted line that they should run `ail verify` for the full Z3 check.
    """
    # Step 1: call ail.status for a lightweight sanity check.
    try:
        result = toolkit.call("ail.status", {})
    except (MCPConnectionError, AgentError) as exc:
        new_state = dict(state)  # type: ignore[arg-type]
        new_state["status"] = "error"
        new_state["error"] = str(exc)
        return new_state  # type: ignore[return-value]

    # Step 2: defensive check — zero nodes means the graph is empty/broken.
    if result.get("node_count", 0) == 0:
        new_state = dict(state)  # type: ignore[arg-type]
        new_state["status"] = "error"
        new_state["error"] = "[AIL-G0140] post-verify status reports zero nodes"
        return new_state  # type: ignore[return-value]

    # Step 3: emit the canonical verification line if a callable was provided.
    if emit is not None:
        emit(VERIFY_OK_LINE)

    # Step 4: shallow-copy state, mark done.
    new_state = dict(state)  # type: ignore[arg-type]
    new_state["status"] = "done"
    new_state["error"] = ""
    return new_state  # type: ignore[return-value]
