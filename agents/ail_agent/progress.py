"""Lightweight progress emitter for AIL agent CLI.

The orchestrator does not import any I/O directly; it routes through a
callable injected via `set_workflow_context(emit=progress.emit, ...)`.
That keeps the workers pure (testable with capturing fakes) while keeping
the user-facing wording in one place.

Two emitter implementations:

- `Progress` — plain-text, one line per call, flushed immediately. This is
  the historic `ail-cli agent` consumer and must remain backwards-compatible.

- `JsonProgress` — structured JSON envelopes on stdout, one per line, wearing
  a `runId` echoed from the CLI flag. Used by the Tauri IDE in Phase 16 task
  16.1 via `python -m ail_agent ... --json-events --run-id <id>`. See
  `docs/plan/v4.0/reference/AIL-Agent-IDE-v4.0.md` for the canonical shape.
"""
from __future__ import annotations

import json
import sys
import uuid
from typing import Any, TextIO

VERIFY_OK_LINE: str = "Basic verification passed. Run ail verify for full Z3 check."


class Progress:
    """Stateless plain-text emitter; instances are interchangeable."""

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


class JsonProgress(Progress):
    """Structured JSON-envelope emitter for the Tauri IDE (Phase 16).

    Emits one JSON object per line to the underlying stream (stdout by
    default). Envelope shapes:

    - step:     ``{"type":"step","runId":"<id>","index":<u32>,"phase":<str>,"text":<str>}``
    - message:  ``{"type":"message","runId":"<id>","messageId":"<uuid>","text":<str>,"preview":<obj|null>}``
    - complete: ``{"type":"complete","runId":"<id>","status":"done"|"error"|"cancelled","error":<str|null>}``

    ``planning/step/verifying/verify_ok`` override `Progress` to emit a step
    envelope with the appropriate `phase` string. `done/error` emit a
    terminal `complete` envelope — the CLI wrapper must still return its
    exit code, but the wire protocol terminates here.

    Tracebacks and `warnings.warn` continue to write to stderr (Python
    default); the Rust reader drains stderr to its own log channel.
    """

    def __init__(self, *, run_id: str, stream: TextIO | None = None) -> None:
        super().__init__(stream=stream)
        self._run_id = run_id
        self._step_index = 0
        self._phase = "plan"

    # --- phase tracking -------------------------------------------------

    def set_phase(self, phase: str) -> None:
        self._phase = phase

    # --- envelope writers ----------------------------------------------

    def _write(self, obj: dict[str, Any]) -> None:
        self._stream.write(json.dumps(obj, separators=(",", ":")) + "\n")
        self._stream.flush()

    def _next_index(self) -> int:
        self._step_index += 1
        return self._step_index

    # --- public protocol ------------------------------------------------

    def emit(self, line: str) -> None:
        self._write(
            {
                "type": "step",
                "runId": self._run_id,
                "index": self._next_index(),
                "phase": self._phase,
                "text": line,
            }
        )

    def planning(self) -> None:
        self.set_phase("plan")
        self.emit("Planning...")

    def step(self, current: int, total: int, intent: str) -> None:
        self.set_phase("code")
        self.emit(f"Step {current}/{total}: {intent}")

    def verifying(self) -> None:
        self.set_phase("verify")
        self.emit("Verifying...")

    def verify_ok(self) -> None:
        self.set_phase("verify")
        self.emit(VERIFY_OK_LINE)

    def done(self) -> None:
        self._write(
            {
                "type": "complete",
                "runId": self._run_id,
                "status": "done",
                "error": None,
            }
        )

    def error(self, message: str) -> None:
        self._write(
            {
                "type": "complete",
                "runId": self._run_id,
                "status": "error",
                "error": message,
            }
        )

    # --- JSON-only helpers ---------------------------------------------

    def message(self, text: str, preview: dict[str, Any] | None = None) -> None:
        """Emit an assistant message envelope with optional preview card."""
        self._write(
            {
                "type": "message",
                "runId": self._run_id,
                "messageId": str(uuid.uuid4()),
                "text": text,
                "preview": preview,
            }
        )


# Module-level default emitter callable for direct injection.
_DEFAULT = Progress()


def emit(line: str) -> None:
    """Default module-level emit hook used by the CLI."""
    _DEFAULT.emit(line)
