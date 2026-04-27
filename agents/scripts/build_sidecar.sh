#!/usr/bin/env bash
# Build the frozen ail-agent binary using PyInstaller.
#
# Usage: bash agents/scripts/build_sidecar.sh
#        (run from repo root or any directory — paths are resolved absolutely)
#
# Prerequisites:
#   pip install -e agents[freeze]   (or pip install -e agents[all,freeze])
#
# Output: agents/dist/ail-agent  (executable, ~150-250 MB)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AGENTS_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# ---------------------------------------------------------------------------
# Detect Python interpreter
# ---------------------------------------------------------------------------
PYTHON=""
if command -v python3 &>/dev/null; then
    PYTHON="python3"
elif command -v python &>/dev/null; then
    PYTHON="python"
else
    echo "ERROR: Python not found on PATH." >&2
    echo "       Install Python 3.10+ and retry." >&2
    exit 1
fi

echo "Using Python: $("$PYTHON" --version)"

# Verify Python version >= 3.10 (matches pyproject.toml requires-python).
if ! "$PYTHON" -c 'import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)' 2>/dev/null; then
    echo "ERROR: Python 3.10+ required (found: $("$PYTHON" --version 2>&1))." >&2
    echo "       Install Python 3.10+ and retry." >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# Verify PyInstaller is importable
# ---------------------------------------------------------------------------
if ! "$PYTHON" -c 'import PyInstaller' 2>/dev/null; then
    echo "ERROR: PyInstaller is not installed." >&2
    echo "       Run: pip install -e agents[freeze]" >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# Run PyInstaller from the agents/ directory
# ---------------------------------------------------------------------------
echo "Building frozen binary from: ${AGENTS_DIR}"
cd "${AGENTS_DIR}"
"$PYTHON" -m PyInstaller --clean --noconfirm ail-agent.spec

# ---------------------------------------------------------------------------
# Verify output
# ---------------------------------------------------------------------------
FROZEN_BIN="${AGENTS_DIR}/dist/ail-agent"
if [ ! -x "${FROZEN_BIN}" ]; then
    echo "ERROR: Expected ${FROZEN_BIN} to exist and be executable after build." >&2
    exit 1
fi

echo "Build succeeded: ${FROZEN_BIN}"
