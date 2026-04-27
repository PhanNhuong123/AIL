"""Non-gated frozen-binary tests — Phase 16.6.

These tests run on every CI matrix leg without any special environment setup.
They verify that:

  t1: the installed package metadata version matches ``ail_agent.__version__``
      (drift guard — mitigates risk R6).
  t2: running ``python -m ail_agent --version`` in dev mode exits 0 and prints
      a version string starting with ``"ail_agent "``.

No frozen binary or API key is required. Tests t3-t5 live in
``test_frozen_binary_smoke.py`` and are gated on ``AIL_FROZEN_BIN``.
"""
from __future__ import annotations

import subprocess
import sys

import importlib.metadata


def test_pyproject_version_matches_dunder() -> None:
    """Package metadata version must equal ``ail_agent.__version__``."""
    import ail_agent  # noqa: PLC0415

    metadata_version = importlib.metadata.version("ail-agent")
    assert metadata_version == ail_agent.__version__, (
        f"importlib.metadata version {metadata_version!r} does not match "
        f"ail_agent.__version__ {ail_agent.__version__!r}. "
        "Update one to match the other."
    )


def test_dev_mode_version_smoke() -> None:
    """``python -m ail_agent --version`` must exit 0 and print a valid header."""
    proc = subprocess.run(
        [sys.executable, "-m", "ail_agent", "--version"],
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
