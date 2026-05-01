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
from ail_agent.progress import JsonProgress, Progress
from ail_agent.registry import get_provider

_DEFAULT_MODEL = "anthropic:claude-sonnet-4-5"


def _force_utf8_io() -> None:
    """Switch stdout/stderr (and on Windows, the console output code page) to
    UTF-8 so non-ASCII characters in help text and JSON envelopes survive
    the platform default (cp1252 on Windows, which mojibakes em-dashes into
    replacement glyphs).

    Two-step sequence:

    1. On Windows, ask the OS to flip the console output code page to 65001
       (UTF-8). This affects how the console renders bytes Python writes;
       without it, even a UTF-8 stream is mis-rendered. ``SetConsoleOutputCP``
       is a no-op when stdout is redirected to a pipe/file, and may fail
       silently inside some terminal emulators — that is acceptable, the
       second step still reduces corruption.
    2. Reconfigure stdout/stderr to encode with UTF-8 + ``errors='replace'``.
       Idempotent and a no-op when the streams do not expose
       ``reconfigure()`` (older runtimes, redirected non-text streams).

    The Tauri IDE sidecar reader already decodes UTF-8 explicitly, so this
    primarily fixes user-visible output when launching ``ail-agent`` from a
    plain Windows console.
    """
    if sys.platform == "win32":
        try:
            import ctypes

            ctypes.windll.kernel32.SetConsoleOutputCP(65001)  # type: ignore[attr-defined]
        except (OSError, AttributeError):
            # Non-windll environment / restricted process — drop silently.
            pass

    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if callable(reconfigure):
            try:
                reconfigure(encoding="utf-8", errors="replace")
            except (ValueError, OSError, AttributeError):
                # Non-text stream / closed / unsupported — drop silently.
                pass


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
    parser.add_argument(
        "--json-events",
        action="store_true",
        dest="json_events",
        help=(
            "Emit structured JSON envelopes on stdout (one per line) instead "
            "of human-readable text. Used by the Tauri IDE sidecar in Phase "
            "16. Requires --run-id to be meaningful for the consumer."
        ),
    )
    parser.add_argument(
        "--run-id",
        default="0",
        dest="run_id",
        metavar="ID",
        help=(
            "Opaque run identifier echoed in every JSON envelope. Only used "
            "when --json-events is set. Default: 0"
        ),
    )
    import ail_agent
    parser.add_argument(
        "--version",
        action="version",
        version=f"ail_agent {ail_agent.__version__}",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Entry point used by both `python -m ail_agent` and the `ail-agent` script.

    Returns the process exit code; the module-level guard calls sys.exit(main()).
    """
    _force_utf8_io()
    args = _build_parser().parse_args(argv)
    progress: Progress = (
        JsonProgress(run_id=args.run_id) if args.json_events else Progress()
    )

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
