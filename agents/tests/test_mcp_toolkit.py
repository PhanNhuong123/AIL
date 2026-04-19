"""Tests for MCPToolkit — synchronous MCP wrapper.

All tests patch ``stdio_client`` and ``ClientSession`` so no real ``ail``
binary is ever spawned.  The fakes use real ``mcp.types`` where the import is
clean, and SimpleNamespace-style objects elsewhere.
"""
from __future__ import annotations

import asyncio
import json
from contextlib import asynccontextmanager
from types import SimpleNamespace
from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from mcp.types import TextContent, Tool
from ail_agent.errors import MCPConnectionError
from ail_agent.mcp_toolkit import MCPToolkit

# ---------------------------------------------------------------------------
# Shared fake helpers
# ---------------------------------------------------------------------------


def _make_call_result(*text_blocks: str) -> SimpleNamespace:
    """Build a fake CallToolResult whose .content is a list of TextContent."""
    content = [TextContent(type="text", text=t) for t in text_blocks]
    return SimpleNamespace(content=content, isError=False)


def _make_list_result(*names: str) -> SimpleNamespace:
    """Build a fake ListToolsResult whose .tools is a list of Tool."""
    tools = [Tool(name=n, inputSchema={}) for n in names]
    return SimpleNamespace(tools=tools)


class _FakeSession:
    """Minimal ClientSession stand-in for happy-path tests."""

    def __init__(self, *, call_result: Any = None, list_result: Any = None) -> None:
        self._call_result = call_result or _make_call_result('{"ok": true}')
        self._list_result = list_result or _make_list_result()
        # Track calls for assertions
        self.call_tool = AsyncMock(return_value=self._call_result)
        self.list_tools = AsyncMock(return_value=self._list_result)
        self.initialize = AsyncMock()

    async def __aenter__(self) -> "_FakeSession":
        return self

    async def __aexit__(self, *_: Any) -> None:
        pass


def _build_patchers(fake_session: "_FakeSession"):
    """Return (stdio_client_patch, ClientSession_patch) context managers."""

    @asynccontextmanager
    async def _fake_stdio_client(_params: Any):
        # Yield two dummy stream objects (read, write)
        yield (object(), object())

    stdio_patch = patch(
        "ail_agent.mcp_toolkit.stdio_client",
        new=_fake_stdio_client,
    )
    # patch inside _connect where the names are imported
    session_patch = patch(
        "ail_agent.mcp_toolkit.ClientSession",
        return_value=fake_session,
    )
    return stdio_patch, session_patch


# ---------------------------------------------------------------------------
# Test 1 — happy path: call returns parsed JSON
# ---------------------------------------------------------------------------


def test_call_returns_parsed_json():
    session = _FakeSession(call_result=_make_call_result('{"ok": true}'))
    stdio_patch, session_patch = _build_patchers(session)

    with stdio_patch, session_patch:
        with MCPToolkit(server_command="ail", server_args=["serve"]) as tk:
            result = tk.call("ail.status", {})

    assert result == {"ok": True}


# ---------------------------------------------------------------------------
# Test 2 — connect timeout raises MCPConnectionError
# ---------------------------------------------------------------------------


def test_connect_timeout_raises_mcp_connection_error():
    @asynccontextmanager
    async def _slow_stdio_client(_params: Any):
        await asyncio.sleep(10)
        yield (object(), object())

    with patch("ail_agent.mcp_toolkit.stdio_client", new=_slow_stdio_client):
        with pytest.raises(MCPConnectionError) as exc_info:
            MCPToolkit(
                server_command="ail",
                server_args=["serve"],
                connect_timeout=0.1,
                port=7777,
            ).__enter__()

    err = exc_info.value
    assert err.port == 7777
    assert "timeout" in str(err).lower()


# ---------------------------------------------------------------------------
# Test 3 — call timeout raises MCPConnectionError
# ---------------------------------------------------------------------------


def test_call_timeout_raises_mcp_connection_error():
    async def _slow_call_tool(*_args: Any, **_kwargs: Any) -> Any:
        await asyncio.sleep(10)

    session = _FakeSession()
    session.call_tool = AsyncMock(side_effect=_slow_call_tool)
    stdio_patch, session_patch = _build_patchers(session)

    with stdio_patch, session_patch:
        with MCPToolkit(server_command="ail", server_args=["serve"]) as tk:
            with pytest.raises(MCPConnectionError) as exc_info:
                tk.call("ail.search", {}, timeout=0.1)

    err = exc_info.value
    err_str = str(err)
    assert "timed out" in err_str
    assert "ail.search" in err_str


# ---------------------------------------------------------------------------
# Test 4 — call after close raises MCPConnectionError mentioning "closed"
# ---------------------------------------------------------------------------


def test_call_after_close_raises():
    session = _FakeSession()
    stdio_patch, session_patch = _build_patchers(session)

    with stdio_patch, session_patch:
        tk = MCPToolkit(server_command="ail", server_args=["serve"]).__enter__()

    # Toolkit is now outside the context manager — __exit__ was NOT called yet.
    # Use explicit close instead.
    tk.close()

    with pytest.raises(MCPConnectionError) as exc_info:
        tk.call("ail.status", {})

    assert "closed" in str(exc_info.value).lower()


# ---------------------------------------------------------------------------
# Test 5 — close is idempotent (no exception on double close)
# ---------------------------------------------------------------------------


def test_close_is_idempotent():
    session = _FakeSession()
    stdio_patch, session_patch = _build_patchers(session)

    with stdio_patch, session_patch:
        tk = MCPToolkit(server_command="ail", server_args=["serve"]).__enter__()

    tk.close()
    tk.close()  # Must not raise


# ---------------------------------------------------------------------------
# Test 6 — list_tools returns tool names
# ---------------------------------------------------------------------------


def test_list_tools_returns_names():
    list_result = _make_list_result("ail.status", "ail.write")
    session = _FakeSession(list_result=list_result)
    stdio_patch, session_patch = _build_patchers(session)

    with stdio_patch, session_patch:
        with MCPToolkit(server_command="ail", server_args=["serve"]) as tk:
            names = tk.list_tools()

    assert names == ["ail.status", "ail.write"]


# ---------------------------------------------------------------------------
# Test 7 — non-JSON response raises MCPConnectionError wrapping JSONDecodeError
# ---------------------------------------------------------------------------


def test_call_non_json_response_raises():
    session = _FakeSession(call_result=_make_call_result("not json"))
    stdio_patch, session_patch = _build_patchers(session)

    with stdio_patch, session_patch:
        with MCPToolkit(server_command="ail", server_args=["serve"]) as tk:
            with pytest.raises(MCPConnectionError) as exc_info:
                tk.call("ail.search", {})

    err = exc_info.value
    assert err.__cause__ is not None
    assert isinstance(err.__cause__, json.JSONDecodeError)


# ---------------------------------------------------------------------------
# Test 8 — context manager enters and toolkit is unusable after the block
# ---------------------------------------------------------------------------


def test_context_manager_enters_and_closes():
    session = _FakeSession()
    stdio_patch, session_patch = _build_patchers(session)

    with stdio_patch, session_patch:
        with MCPToolkit(server_command="ail", server_args=["serve"]) as tk:
            # Toolkit is usable inside the block
            result = tk.call("ail.status", {})
            assert result == {"ok": True}

    # After the context block the toolkit must refuse new calls
    with pytest.raises(MCPConnectionError) as exc_info:
        tk.call("ail.status", {})

    assert "closed" in str(exc_info.value).lower()
