"""python -m ail_agent — user-facing entry point."""
from __future__ import annotations

import argparse
import sys
from typing import Sequence

from ail_agent.errors import AgentError, MCPConnectionError, ProviderConfigError
from ail_agent.mcp_toolkit import MCPToolkit
from ail_agent.orchestrator import (
    build_workflow,
    clear_workflow_context,
    initial_state,
    set_workflow_context,
)
from ail_agent.progress import Progress
from ail_agent.registry import get_provider

_DEFAULT_MODEL = "anthropic:claude-sonnet-4-5"


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="ail_agent",
        description="AIL agent — drive the Rust MCP pipeline via plan/code/verify loops.",
    )
    parser.add_argument(
        "task",
        help="Developer task description to execute.",
    )
    parser.add_argument(
        "--model",
        default=_DEFAULT_MODEL,
        metavar="PREFIX:NAME",
        help=f"Provider and model spec, e.g. anthropic:claude-sonnet-4-5. Default: {_DEFAULT_MODEL}",
    )
    parser.add_argument(
        "--mcp-port",
        type=int,
        default=7777,
        dest="mcp_port",
        metavar="PORT",
        help="MCP port (reserved; current impl uses stdio). Default: 7777",
    )
    parser.add_argument(
        "--max-iterations",
        type=int,
        default=50,
        dest="max_iterations",
        metavar="N",
        help="Maximum workflow iterations before aborting. Default: 50",
    )
    parser.add_argument(
        "--steps-per-plan",
        type=int,
        default=20,
        dest="steps_per_plan",
        metavar="N",
        help="Maximum plan steps per coder pass. Default: 20",
    )
    parser.add_argument(
        "--ail-bin",
        default="ail",
        dest="ail_bin",
        metavar="PATH",
        help="Path to the `ail` binary or command name for PATH lookup. Default: ail",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Entry point used by both `python -m ail_agent` and the `ail-agent` script.

    Returns the process exit code; the module-level guard calls sys.exit(main()).
    """
    args = _build_parser().parse_args(argv)
    progress = Progress()

    # Resolve provider early so config errors surface before any MCP connection.
    try:
        provider, model = get_provider(args.model)
    except ProviderConfigError as exc:
        progress.error(str(exc))
        return 2

    try:
        with MCPToolkit(
            server_command=args.ail_bin,
            server_args=["serve"],
            port=args.mcp_port,
        ) as toolkit:
            set_workflow_context(
                provider=provider,
                model=model,
                toolkit=toolkit,
                emit=progress.emit,
            )
            try:
                state = initial_state(
                    task=args.task,
                    model=args.model,
                    mcp_port=args.mcp_port,
                    max_iterations=args.max_iterations,
                    steps_per_plan=args.steps_per_plan,
                )
                graph = build_workflow()
                # Use .invoke() which returns the final state directly.
                # This is simpler and more reliable than .stream() for capturing
                # the terminal state; tests mock build_workflow() so the choice
                # is transparent to the test layer.
                final_state = graph.invoke(state)
                if final_state.get("status") == "done":
                    progress.done()
                    return 0
                else:
                    err = final_state.get("error") or "(no error message)"
                    progress.error(err)
                    return 1
            finally:
                clear_workflow_context()
    except MCPConnectionError as exc:
        progress.error(str(exc))
        return 3
    except KeyboardInterrupt:
        progress.error("interrupted")
        return 130
    except AgentError as exc:
        progress.error(str(exc))
        return 1


if __name__ == "__main__":
    sys.exit(main())
