# Build the frozen ail-agent binary using PyInstaller (Windows).
#
# Usage: pwsh agents/scripts/build_sidecar.ps1
#        (run from repo root or any directory — paths are resolved absolutely)
#
# Prerequisites:
#   pip install -e agents[freeze]   (or pip install -e agents[all,freeze])
#
# Output: agents\dist\ail-agent.exe  (executable, ~150-250 MB)

$ErrorActionPreference = 'Stop'

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$AgentsDir = Resolve-Path "$ScriptDir/.."

# ---------------------------------------------------------------------------
# Detect Python interpreter
# ---------------------------------------------------------------------------
$Python = $null
foreach ($candidate in @('python', 'py')) {
    if (Get-Command $candidate -ErrorAction SilentlyContinue) {
        $Python = $candidate
        break
    }
}
if (-not $Python) {
    Write-Error "Python not found on PATH. Install Python 3.10+ and retry."
    exit 1
}

$pyVer = & $Python --version
Write-Host "Using Python: $pyVer"

# Verify Python version >= 3.10 (matches pyproject.toml requires-python).
& $Python -c 'import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)' 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Error "Python 3.10+ required (found: $pyVer). Install Python 3.10+ and retry."
    exit 1
}

# ---------------------------------------------------------------------------
# Verify PyInstaller is importable
# ---------------------------------------------------------------------------
& $Python -c 'import PyInstaller' 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Error "PyInstaller is not installed. Run: pip install -e agents[freeze]"
    exit 1
}

# ---------------------------------------------------------------------------
# Run PyInstaller from the agents/ directory
# ---------------------------------------------------------------------------
Write-Host "Building frozen binary from: $AgentsDir"
Push-Location $AgentsDir
try {
    & $Python -m PyInstaller --clean --noconfirm ail-agent.spec
    if ($LASTEXITCODE -ne 0) {
        Write-Error "PyInstaller exited with code $LASTEXITCODE"
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}

# ---------------------------------------------------------------------------
# Verify output
# ---------------------------------------------------------------------------
$FrozenBin = Join-Path $AgentsDir "dist\ail-agent.exe"
if (-not (Test-Path $FrozenBin)) {
    Write-Error "Expected $FrozenBin to exist after build."
    exit 1
}

$frozenSize = (Get-Item $FrozenBin).Length
if ($frozenSize -lt 1024) {
    Write-Error "Frozen binary $FrozenBin is suspiciously small ($frozenSize bytes); build likely failed silently."
    exit 1
}

Write-Host "Build succeeded: $FrozenBin"
