# PyInstaller spec for the ail-agent frozen binary — Phase 16.6.
#
# Run this spec from the `agents/` directory:
#
#   cd agents
#   python -m PyInstaller --clean --noconfirm ail-agent.spec
#
# Output: agents/dist/ail-agent[.exe]  (single-file, console=True)
#
# All paths in this spec are relative to `agents/` (the spec's cwd).
# Do NOT invoke this from the repo root; paths will be wrong.

from PyInstaller.utils.hooks import collect_data_files

block_cipher = None

a = Analysis(
    ["ail_agent/__main__.py"],
    pathex=["."],
    binaries=[],
    datas=collect_data_files("certifi") + [("ail_agent", "ail_agent")],
    hiddenimports=[
        # All 5 provider modules — imported lazily via importlib.import_module,
        # so PyInstaller's static analysis misses them (risk R1 HIGH).
        "ail_agent.providers.anthropic",
        "ail_agent.providers.openai",
        "ail_agent.providers.deepseek",
        "ail_agent.providers.alibaba",
        "ail_agent.providers.ollama",
        "ail_agent.providers.base",
        "ail_agent.providers._openai_compat",
        "ail_agent.providers._retry",
        # LangGraph + LangChain core — deep import trees that PyInstaller
        # may partially miss due to dynamic plugin/registry patterns.
        "langgraph",
        "langgraph.graph",
        "langgraph.checkpoint",
        "langchain_core",
        "langchain_mcp_adapters",
        # MCP client stack.
        "mcp",
        "mcp.client",
        "mcp.client.stdio",
        # anyio asyncio backend — needed at runtime for anyio.run().
        "anyio._backends._asyncio",
        # certifi — must be importable so SSL cert path resolves (risk R4 HIGH).
        "certifi",
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name="ail-agent",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    onefile=True,
)
