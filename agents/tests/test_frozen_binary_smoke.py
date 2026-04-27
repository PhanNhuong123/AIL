"""Gated frozen-binary smoke tests — Phase 16.6.

These tests require the ``AIL_FROZEN_BIN`` environment variable to point at a
built frozen ail-agent binary (produced by ``agents/scripts/build_sidecar.sh``
or ``build_sidecar.ps1``).  They are skipped silently on developer machines
that have not built the frozen binary.

  t3: frozen binary ``--version`` exits 0 and prints a valid header.
  t4: frozen binary ``--json-events`` emits valid JSON on stdout;
      the last envelope has ``type == "complete"``.
  t5: dev-mode and frozen-binary ``--version`` outputs are byte-identical.

Run all three with::

    AIL_FROZEN_BIN=$PWD/agents/dist/ail-agent \\
        python -m pytest agents/tests/test_frozen_binary_smoke.py -v -m frozen_binary
"""
from __future__ import annotations

import json
import os
import subprocess
import sys

import pytest

AIL_FROZEN_BIN = os.environ.get("AIL_FROZEN_BIN", "")
AIL_FROZEN_BIN_SET = bool(AIL_FROZEN_BIN) and os.path.exists(AIL_FROZEN_BIN)

pytestmark = [
    pytest.mark.integration,
    pytest.mark.frozen_binary,
    pytest.mark.skipif(
        not AIL_FROZEN_BIN_SET,
        reason="AIL_FROZEN_BIN env var not set or binary does not exist",
    ),
]


def test_frozen_binary_version_smoke() -> None:
    """Frozen binary ``--version`` must exit 0 and print a valid header."""
    proc = subprocess.run(
        [AIL_FROZEN_BIN, "--version"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert proc.returncode == 0, (
        f"Expected exit 0, got {proc.returncode}.\n"
        f"stdout: {proc.stdout!r}\n"
        f"stderr: {proc.stderr!r}"
    )
    assert proc.stdout.startswith("ail_agent "), (
        f"Expected stdout to start with 'ail_agent ', got: {proc.stdout!r}"
    )


def test_frozen_binary_json_events_smoke() -> None:
    """Frozen binary ``--json-events`` must emit valid JSON on every stdout line.

    The final envelope must have ``type == "complete"``.  Exit code may be
    nonzero (no API key in CI) — that is acceptable; what matters is that the
    JSON stream is well-formed and terminates with a ``complete`` envelope.
    """
    proc = subprocess.run(
        [AIL_FROZEN_BIN, "smoke task", "--json-events", "--run-id", "smoke"],
        capture_output=True,
        text=True,
        timeout=60,
    )
    # Collect non-empty output lines.
    lines = [line for line in proc.stdout.splitlines() if line.strip()]
    assert lines, (
        "Frozen binary produced no stdout lines.\n"
        f"stderr: {proc.stderr!r}"
    )
    # Every non-empty line must parse as JSON.
    parsed: list[dict] = []
    for i, line in enumerate(lines):
        try:
            obj = json.loads(line)
        except json.JSONDecodeError as exc:
            pytest.fail(
                f"Line {i} is not valid JSON: {line!r}\nJSONDecodeError: {exc}"
            )
        parsed.append(obj)
    # Last envelope must be a ``complete`` event.
    last = parsed[-1]
    assert last.get("type") == "complete", (
        f"Expected last envelope type to be 'complete', got: {last!r}"
    )


def test_dev_frozen_parity_version() -> None:
    """Dev-mode and frozen-binary ``--version`` outputs must be byte-identical."""
    dev_proc = subprocess.run(
        [sys.executable, "-m", "ail_agent", "--version"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    frozen_proc = subprocess.run(
        [AIL_FROZEN_BIN, "--version"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert dev_proc.returncode == 0, f"Dev mode --version failed: {dev_proc.stderr!r}"
    assert frozen_proc.returncode == 0, (
        f"Frozen --version failed: {frozen_proc.stderr!r}"
    )
    assert dev_proc.stdout == frozen_proc.stdout, (
        f"Version output mismatch.\n"
        f"dev:    {dev_proc.stdout!r}\n"
        f"frozen: {frozen_proc.stdout!r}"
    )
