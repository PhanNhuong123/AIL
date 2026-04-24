"""Tests for the Phase 16 `JsonProgress` structured envelope emitter.

Covers:
- Step envelope shape and runId echo.
- Terminal complete/error envelope shape.
- Per-line flushing so the Rust reader gets events incrementally.
- Non-regression: `Progress` plain-text output unchanged.
"""
from __future__ import annotations

import io
import json

import pytest

from ail_agent.progress import JsonProgress, Progress, VERIFY_OK_LINE


def _lines(stream: io.StringIO) -> list[dict]:
    return [json.loads(line) for line in stream.getvalue().splitlines() if line]


# ---------------------------------------------------------------------------
# Step envelope
# ---------------------------------------------------------------------------


def test_json_progress_step_emits_valid_envelope_with_run_id_echo() -> None:
    stream = io.StringIO()
    p = JsonProgress(run_id="r-7", stream=stream)

    p.planning()
    p.step(1, 3, "plan rule X")
    p.verifying()

    events = _lines(stream)
    assert len(events) == 3

    assert events[0]["type"] == "step"
    assert events[0]["runId"] == "r-7"
    assert events[0]["index"] == 1
    assert events[0]["phase"] == "plan"
    assert events[0]["text"] == "Planning..."

    assert events[1]["phase"] == "code"
    assert events[1]["index"] == 2
    assert events[1]["text"] == "Step 1/3: plan rule X"

    assert events[2]["phase"] == "verify"
    assert events[2]["text"] == "Verifying..."


def test_json_progress_verify_ok_uses_verify_phase() -> None:
    stream = io.StringIO()
    p = JsonProgress(run_id="r-42", stream=stream)

    p.verify_ok()

    events = _lines(stream)
    assert events[0]["phase"] == "verify"
    assert events[0]["text"] == VERIFY_OK_LINE


# ---------------------------------------------------------------------------
# Complete envelope
# ---------------------------------------------------------------------------


def test_json_progress_done_emits_complete_done() -> None:
    stream = io.StringIO()
    p = JsonProgress(run_id="r-9", stream=stream)

    p.done()

    events = _lines(stream)
    assert events == [
        {"type": "complete", "runId": "r-9", "status": "done", "error": None}
    ]


def test_json_progress_complete_error_shape() -> None:
    stream = io.StringIO()
    p = JsonProgress(run_id="r-9", stream=stream)

    p.error("boom")

    events = _lines(stream)
    assert events == [
        {"type": "complete", "runId": "r-9", "status": "error", "error": "boom"}
    ]


# ---------------------------------------------------------------------------
# Message envelope
# ---------------------------------------------------------------------------


def test_json_progress_message_without_preview() -> None:
    stream = io.StringIO()
    p = JsonProgress(run_id="r-1", stream=stream)

    p.message("here is the plan")

    events = _lines(stream)
    assert events[0]["type"] == "message"
    assert events[0]["runId"] == "r-1"
    assert events[0]["text"] == "here is the plan"
    assert events[0]["preview"] is None
    assert "messageId" in events[0] and events[0]["messageId"]


def test_json_progress_message_with_preview() -> None:
    stream = io.StringIO()
    p = JsonProgress(run_id="r-1", stream=stream)
    preview = {"title": "Add rate limiter", "summary": "one step", "patch": None}

    p.message("I propose adding a step", preview=preview)

    events = _lines(stream)
    assert events[0]["preview"] == preview


# ---------------------------------------------------------------------------
# Per-line flush
# ---------------------------------------------------------------------------


class _FlushCounter(io.StringIO):
    def __init__(self) -> None:
        super().__init__()
        self.flush_count = 0

    def flush(self) -> None:  # type: ignore[override]
        self.flush_count += 1
        super().flush()


def test_json_progress_flushes_per_line() -> None:
    stream = _FlushCounter()
    p = JsonProgress(run_id="r-2", stream=stream)

    p.planning()
    p.step(1, 1, "x")
    p.done()

    assert stream.flush_count == 3


def test_json_progress_lines_end_with_newline() -> None:
    stream = io.StringIO()
    p = JsonProgress(run_id="r-2", stream=stream)

    p.planning()
    p.done()

    raw = stream.getvalue()
    # Two newline-terminated envelopes; no stray content after the last \n.
    assert raw.count("\n") == 2
    assert raw.endswith("\n")


# ---------------------------------------------------------------------------
# Regression: plain-text Progress output unchanged
# ---------------------------------------------------------------------------


def test_plain_progress_unchanged_under_phase_16() -> None:
    stream = io.StringIO()
    p = Progress(stream=stream)

    p.planning()
    p.step(2, 5, "intent")
    p.verify_ok()
    p.done()
    p.error("oops")

    assert stream.getvalue().splitlines() == [
        "Planning...",
        "Step 2/5: intent",
        VERIFY_OK_LINE,
        "Done.",
        "Error: oops",
    ]


# ---------------------------------------------------------------------------
# Envelope type coverage
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "action, expected_type",
    [
        (lambda p: p.planning(), "step"),
        (lambda p: p.step(1, 1, "x"), "step"),
        (lambda p: p.verifying(), "step"),
        (lambda p: p.verify_ok(), "step"),
        (lambda p: p.done(), "complete"),
        (lambda p: p.error("e"), "complete"),
        (lambda p: p.message("hi"), "message"),
    ],
)
def test_json_progress_envelope_types(action, expected_type) -> None:
    stream = io.StringIO()
    p = JsonProgress(run_id="r-t", stream=stream)
    action(p)
    events = _lines(stream)
    assert events[0]["type"] == expected_type
