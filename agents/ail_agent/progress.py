"""Lightweight progress emitter for AIL agent CLI.

The orchestrator does not import any I/O directly; it routes through a
callable injected via `set_workflow_context(emit=progress.emit, ...)`.
That keeps the workers pure (testable with capturing fakes) while keeping
the user-facing wording in one place.
"""
from __future__ import annotations
import sys
from typing import TextIO

VERIFY_OK_LINE: str = "Basic verification passed. Run ail verify for full Z3 check."


class Progress:
    """Stateless emitter; instances are interchangeable."""

    def __init__(self, *, stream: TextIO | None = None) -> None:
        self._stream = stream if stream is not None else sys.stdout

    def emit(self, line: str) -> None:
        """Print a single line, flush immediately so streaming works."""
        print(line, file=self._stream, flush=True)

    # Convenience helpers — keep wording tight and stable, the CLI tests will lock these.
    def planning(self) -> None:
        self.emit("Planning...")

    def step(self, current: int, total: int, intent: str) -> None:
        self.emit(f"Step {current}/{total}: {intent}")

    def verifying(self) -> None:
        self.emit("Verifying...")

    def verify_ok(self) -> None:
        self.emit(VERIFY_OK_LINE)

    def done(self) -> None:
        self.emit("Done.")

    def error(self, message: str) -> None:
        self.emit(f"Error: {message}")


# Module-level default emitter callable for direct injection.
_DEFAULT = Progress()


def emit(line: str) -> None:
    """Default module-level emit hook used by the CLI."""
    _DEFAULT.emit(line)
