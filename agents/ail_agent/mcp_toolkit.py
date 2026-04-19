"""Synchronous wrapper around the async mcp SDK.

Owns a dedicated background event loop so the planner/coder/verify nodes
(which are sync LangGraph state functions) can call MCP tools without
restructuring the orchestrator.
"""
from __future__ import annotations

import asyncio
import json
import logging
import threading
from contextlib import AbstractContextManager, AsyncExitStack
from concurrent.futures import Future
from typing import Any

from mcp import ClientSession
from mcp.client.stdio import stdio_client, StdioServerParameters

from ail_agent.errors import MCPConnectionError

logger = logging.getLogger(__name__)

DEFAULT_CONNECT_TIMEOUT_SEC: float = 5.0
DEFAULT_CALL_TIMEOUT_SEC: float = 30.0

# Sentinel used to signal the _connect coroutine completed successfully.
_CONNECTED = object()


class MCPToolkit(AbstractContextManager["MCPToolkit"]):
    """Synchronous facade over an async MCP ClientSession.

    Usage:
        with MCPToolkit(server_command="ail", server_args=["serve"]) as tk:
            result = tk.call("ail.status", {})

    Internally:
      - Starts a daemon thread running its own asyncio loop.
      - Opens stdio_client + ClientSession on that loop, with 5s connect timeout.
      - Each .call() submits a coroutine via asyncio.run_coroutine_threadsafe
        and blocks the calling thread on the resulting Future.
      - On context exit, gracefully closes the session, the stdio child, and
        stops the loop.
    """

    def __init__(
        self,
        *,
        server_command: str = "ail",
        server_args: list[str] | None = None,
        env: dict[str, str] | None = None,
        cwd: str | None = None,
        port: int | None = None,  # Reserved; current impl uses stdio.
        connect_timeout: float = DEFAULT_CONNECT_TIMEOUT_SEC,
        call_timeout: float = DEFAULT_CALL_TIMEOUT_SEC,
    ) -> None:
        self._server_command = server_command
        self._server_args = server_args or []
        self._env = env
        self._cwd = cwd
        self._port = port
        self._connect_timeout = connect_timeout
        self._call_timeout = call_timeout

        self._loop: asyncio.AbstractEventLoop | None = None
        self._thread: threading.Thread | None = None
        self._session: "ClientSession | None" = None
        self._exit_stack: AsyncExitStack | None = None
        self._closed = False

    # ------------------------------------------------------------------
    # Context manager protocol
    # ------------------------------------------------------------------

    def __enter__(self) -> "MCPToolkit":
        self._loop = asyncio.new_event_loop()
        self._thread = threading.Thread(
            target=self._loop.run_forever,
            name="mcp-toolkit-loop",
            daemon=True,
        )
        self._thread.start()

        connect_future: "Future[object]" = asyncio.run_coroutine_threadsafe(
            self._connect(), self._loop
        )
        try:
            connect_future.result(timeout=self._connect_timeout)
        except TimeoutError as exc:
            self._stop_loop()
            raise MCPConnectionError(
                f"connect timeout after {self._connect_timeout}s",
                port=self._port,
                cause=exc,
            ) from exc
        except MCPConnectionError:
            self._stop_loop()
            raise
        except Exception as exc:
            self._stop_loop()
            raise MCPConnectionError(
                f"failed to connect: {exc}",
                port=self._port,
                cause=exc,
            ) from exc

        return self

    def __exit__(self, exc_type: Any, exc: Any, tb: Any) -> None:
        self.close()

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def call(
        self,
        tool_name: str,
        arguments: dict[str, Any] | None = None,
        *,
        timeout: float | None = None,
    ) -> dict[str, Any]:
        """Call an MCP tool by name; return the structured result content as a dict.

        Raises MCPConnectionError if the toolkit is not connected (connect failed
        or close was called) or if the underlying call times out.
        """
        if self._session is None or self._closed:
            raise MCPConnectionError(
                "toolkit is closed",
                port=self._port,
            )

        effective_timeout = timeout if timeout is not None else self._call_timeout

        call_future: "Future[Any]" = asyncio.run_coroutine_threadsafe(
            self._session.call_tool(tool_name, arguments=arguments or {}),
            self._loop,  # type: ignore[arg-type]
        )
        try:
            result = call_future.result(timeout=effective_timeout)
        except TimeoutError as exc:
            raise MCPConnectionError(
                f"call to {tool_name!r} timed out after {effective_timeout}s",
                port=self._port,
                cause=exc,
            ) from exc
        except MCPConnectionError:
            raise
        except Exception as exc:
            raise MCPConnectionError(
                f"call to {tool_name!r} failed: {exc}",
                port=self._port,
                cause=exc,
            ) from exc

        return self._extract_json(tool_name, result)

    def list_tools(self) -> list[str]:
        """Return the names of tools advertised by the server."""
        if self._session is None or self._closed:
            raise MCPConnectionError(
                "toolkit is closed",
                port=self._port,
            )

        list_future: "Future[Any]" = asyncio.run_coroutine_threadsafe(
            self._session.list_tools(),
            self._loop,  # type: ignore[arg-type]
        )
        try:
            result = list_future.result(timeout=self._call_timeout)
        except TimeoutError as exc:
            raise MCPConnectionError(
                f"list_tools timed out after {self._call_timeout}s",
                port=self._port,
                cause=exc,
            ) from exc
        except Exception as exc:
            raise MCPConnectionError(
                f"list_tools failed: {exc}",
                port=self._port,
                cause=exc,
            ) from exc

        return [tool.name for tool in result.tools]

    def close(self) -> None:
        """Idempotent cleanup; called by __exit__."""
        if self._closed:
            return
        self._closed = True

        if self._exit_stack is not None and self._loop is not None and self._loop.is_running():
            close_future: "Future[None]" = asyncio.run_coroutine_threadsafe(
                self._exit_stack.aclose(),
                self._loop,
            )
            try:
                close_future.result(timeout=5.0)
            except Exception as exc:
                logger.debug("MCPToolkit: error during async close: %s", exc)

        self._session = None
        self._exit_stack = None
        self._stop_loop()

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    async def _connect(self) -> object:
        """Open the stdio child process and initialize the MCP session.

        Stores the ClientSession and AsyncExitStack on self so they remain
        alive for the lifetime of the toolkit.
        """
        params = StdioServerParameters(
            command=self._server_command,
            args=self._server_args,
            env=self._env,
            cwd=self._cwd,
        )

        stack = AsyncExitStack()
        try:
            read, write = await stack.enter_async_context(stdio_client(params))
            session: "ClientSession" = await stack.enter_async_context(
                ClientSession(read, write)
            )
            await session.initialize()
        except Exception:
            await stack.aclose()
            raise

        self._exit_stack = stack
        self._session = session
        return _CONNECTED

    @staticmethod
    def _extract_json(tool_name: str, result: Any) -> dict[str, Any]:
        """Concatenate TextContent blocks from a CallToolResult and JSON-parse them."""
        content_blocks = getattr(result, "content", []) or []
        text_parts: list[str] = []
        for block in content_blocks:
            block_type = getattr(block, "type", None)
            if block_type == "text":
                text = getattr(block, "text", "")
                if text:
                    text_parts.append(text)

        joined = "".join(text_parts)
        try:
            parsed = json.loads(joined)
        except (json.JSONDecodeError, ValueError) as exc:
            raise MCPConnectionError(
                f"non-JSON response from {tool_name!r}: {joined!r}",
                cause=exc,
            ) from exc

        if not isinstance(parsed, dict):
            raise MCPConnectionError(
                f"non-dict JSON response from {tool_name!r}: expected object, got {type(parsed).__name__}",
            )

        return parsed

    def _stop_loop(self) -> None:
        """Signal the event loop to stop and join the thread."""
        if self._loop is not None:
            try:
                self._loop.call_soon_threadsafe(self._loop.stop)
            except RuntimeError:
                pass
        if self._thread is not None:
            self._thread.join(timeout=2.0)
        self._loop = None
        self._thread = None
