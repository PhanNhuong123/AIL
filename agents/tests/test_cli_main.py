"""pytest tests for the ail_agent.__main__ CLI entry point (task 14.3 step 12)."""
from __future__ import annotations

import sys
from io import StringIO
from typing import Any
from unittest.mock import MagicMock, Mock, call, patch

import pytest

from ail_agent.__main__ import _force_utf8_io, main
from ail_agent.errors import AgentError, MCPConnectionError, ProviderConfigError


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _mock_provider():
    """Return a mock (provider, model) pair."""
    return Mock(), "claude-sonnet-4-5"


def _make_graph_mock(final_status: str = "done", error: str | None = None) -> Mock:
    """Return a mock compiled graph whose .invoke() returns the desired terminal state."""
    graph = MagicMock()
    result: dict[str, Any] = {"status": final_status, "error": error}
    graph.invoke.return_value = result
    return graph


# ---------------------------------------------------------------------------
# 1. --help exits 0
# ---------------------------------------------------------------------------

def test_main_help_exits_zero():
    with pytest.raises(SystemExit) as exc_info:
        main(["--help"])
    assert exc_info.value.code == 0


# ---------------------------------------------------------------------------
# 2. Missing task argument exits non-zero
# ---------------------------------------------------------------------------

def test_main_missing_task_exits_nonzero():
    with pytest.raises(SystemExit) as exc_info:
        main([])
    assert exc_info.value.code != 0


# ---------------------------------------------------------------------------
# 3. Returns 0 when workflow ends with status="done"
# ---------------------------------------------------------------------------

@patch("ail_agent.__main__.build_workflow")
@patch("ail_agent.__main__.set_workflow_context")
@patch("ail_agent.__main__.clear_workflow_context")
@patch("ail_agent.__main__.MCPToolkit")
@patch("ail_agent.__main__.get_provider")
def test_main_returns_0_on_done(
    mock_get_provider,
    mock_mcp_cls,
    mock_clear,
    mock_set_ctx,
    mock_build,
):
    mock_get_provider.return_value = _mock_provider()
    toolkit_instance = MagicMock()
    mock_mcp_cls.return_value.__enter__ = Mock(return_value=toolkit_instance)
    mock_mcp_cls.return_value.__exit__ = Mock(return_value=False)
    mock_build.return_value = _make_graph_mock("done")

    result = main(["do something"])

    assert result == 0


# ---------------------------------------------------------------------------
# 4. Returns 1 when workflow ends with status="error"
# ---------------------------------------------------------------------------

@patch("ail_agent.__main__.build_workflow")
@patch("ail_agent.__main__.set_workflow_context")
@patch("ail_agent.__main__.clear_workflow_context")
@patch("ail_agent.__main__.MCPToolkit")
@patch("ail_agent.__main__.get_provider")
def test_main_returns_1_on_error_status(
    mock_get_provider,
    mock_mcp_cls,
    mock_clear,
    mock_set_ctx,
    mock_build,
):
    mock_get_provider.return_value = _mock_provider()
    toolkit_instance = MagicMock()
    mock_mcp_cls.return_value.__enter__ = Mock(return_value=toolkit_instance)
    mock_mcp_cls.return_value.__exit__ = Mock(return_value=False)
    mock_build.return_value = _make_graph_mock("error", error="something went wrong")

    result = main(["do something"])

    assert result == 1


# ---------------------------------------------------------------------------
# 5. Returns 2 on ProviderConfigError
# ---------------------------------------------------------------------------

@patch("ail_agent.__main__.get_provider", side_effect=ProviderConfigError("bad config"))
def test_main_returns_2_on_provider_config_error(mock_get_provider):
    result = main(["do something"])
    assert result == 2


# ---------------------------------------------------------------------------
# 6. Returns 3 on MCPConnectionError from __enter__
# ---------------------------------------------------------------------------

@patch("ail_agent.__main__.MCPToolkit")
@patch("ail_agent.__main__.get_provider")
def test_main_returns_3_on_mcp_connection_error(mock_get_provider, mock_mcp_cls):
    mock_get_provider.return_value = _mock_provider()
    mock_mcp_cls.return_value.__enter__ = Mock(
        side_effect=MCPConnectionError("cannot connect")
    )
    mock_mcp_cls.return_value.__exit__ = Mock(return_value=False)

    result = main(["do something"])

    assert result == 3


# ---------------------------------------------------------------------------
# 7. Returns 130 on KeyboardInterrupt
# ---------------------------------------------------------------------------

@patch("ail_agent.__main__.build_workflow")
@patch("ail_agent.__main__.set_workflow_context")
@patch("ail_agent.__main__.clear_workflow_context")
@patch("ail_agent.__main__.MCPToolkit")
@patch("ail_agent.__main__.get_provider")
def test_main_returns_130_on_keyboard_interrupt(
    mock_get_provider,
    mock_mcp_cls,
    mock_clear,
    mock_set_ctx,
    mock_build,
):
    mock_get_provider.return_value = _mock_provider()
    toolkit_instance = MagicMock()
    mock_mcp_cls.return_value.__enter__ = Mock(return_value=toolkit_instance)
    mock_mcp_cls.return_value.__exit__ = Mock(return_value=False)
    # Raise KeyboardInterrupt inside graph.invoke
    graph_mock = MagicMock()
    graph_mock.invoke.side_effect = KeyboardInterrupt
    mock_build.return_value = graph_mock

    result = main(["do something"])

    assert result == 130


# ---------------------------------------------------------------------------
# 8. CLI args are passed through to initial_state
# ---------------------------------------------------------------------------

@patch("ail_agent.__main__.initial_state")
@patch("ail_agent.__main__.build_workflow")
@patch("ail_agent.__main__.set_workflow_context")
@patch("ail_agent.__main__.clear_workflow_context")
@patch("ail_agent.__main__.MCPToolkit")
@patch("ail_agent.__main__.get_provider")
def test_main_passes_args_to_initial_state(
    mock_get_provider,
    mock_mcp_cls,
    mock_clear,
    mock_set_ctx,
    mock_build,
    mock_initial_state,
):
    mock_get_provider.return_value = _mock_provider()
    toolkit_instance = MagicMock()
    mock_mcp_cls.return_value.__enter__ = Mock(return_value=toolkit_instance)
    mock_mcp_cls.return_value.__exit__ = Mock(return_value=False)
    # initial_state returns a dict with status=done so flow completes
    fake_state: dict[str, Any] = {"status": "done", "error": None}
    mock_initial_state.return_value = fake_state
    graph_mock = MagicMock()
    graph_mock.invoke.return_value = {"status": "done", "error": None}
    mock_build.return_value = graph_mock

    result = main([
        "my task",
        "--max-iterations", "99",
        "--steps-per-plan", "7",
    ])

    assert result == 0
    mock_initial_state.assert_called_once()
    _, kwargs = mock_initial_state.call_args
    assert kwargs.get("max_iterations") == 99 or mock_initial_state.call_args[0][2] == 99  # noqa: E501
    # Check via keyword args (the expected call signature)
    call_kwargs = mock_initial_state.call_args.kwargs
    assert call_kwargs["max_iterations"] == 99
    assert call_kwargs["steps_per_plan"] == 7


# ---------------------------------------------------------------------------
# 9. clear_workflow_context is called on success
# ---------------------------------------------------------------------------

@patch("ail_agent.__main__.build_workflow")
@patch("ail_agent.__main__.set_workflow_context")
@patch("ail_agent.__main__.clear_workflow_context")
@patch("ail_agent.__main__.MCPToolkit")
@patch("ail_agent.__main__.get_provider")
def test_main_clears_workflow_context_on_success(
    mock_get_provider,
    mock_mcp_cls,
    mock_clear,
    mock_set_ctx,
    mock_build,
):
    mock_get_provider.return_value = _mock_provider()
    toolkit_instance = MagicMock()
    mock_mcp_cls.return_value.__enter__ = Mock(return_value=toolkit_instance)
    mock_mcp_cls.return_value.__exit__ = Mock(return_value=False)
    mock_build.return_value = _make_graph_mock("done")

    main(["do something"])

    mock_clear.assert_called_once()


# ---------------------------------------------------------------------------
# 10. clear_workflow_context is called on error path
# ---------------------------------------------------------------------------

@patch("ail_agent.__main__.build_workflow")
@patch("ail_agent.__main__.set_workflow_context")
@patch("ail_agent.__main__.clear_workflow_context")
@patch("ail_agent.__main__.MCPToolkit")
@patch("ail_agent.__main__.get_provider")
def test_main_clears_workflow_context_on_error(
    mock_get_provider,
    mock_mcp_cls,
    mock_clear,
    mock_set_ctx,
    mock_build,
):
    mock_get_provider.return_value = _mock_provider()
    toolkit_instance = MagicMock()
    mock_mcp_cls.return_value.__enter__ = Mock(return_value=toolkit_instance)
    mock_mcp_cls.return_value.__exit__ = Mock(return_value=False)
    mock_build.return_value = _make_graph_mock("error", error="boom")

    main(["do something"])

    mock_clear.assert_called_once()


# ---------------------------------------------------------------------------
# 11. Progress emits lines in expected order (done path)
# ---------------------------------------------------------------------------

@patch("ail_agent.__main__.build_workflow")
@patch("ail_agent.__main__.set_workflow_context")
@patch("ail_agent.__main__.clear_workflow_context")
@patch("ail_agent.__main__.MCPToolkit")
@patch("ail_agent.__main__.get_provider")
def test_main_streams_progress_done(
    mock_get_provider,
    mock_mcp_cls,
    mock_clear,
    mock_set_ctx,
    mock_build,
):
    mock_get_provider.return_value = _mock_provider()
    toolkit_instance = MagicMock()
    mock_mcp_cls.return_value.__enter__ = Mock(return_value=toolkit_instance)
    mock_mcp_cls.return_value.__exit__ = Mock(return_value=False)
    mock_build.return_value = _make_graph_mock("done")

    stream = StringIO()
    with patch("ail_agent.__main__.Progress") as mock_progress_cls:
        progress_instance = MagicMock()
        mock_progress_cls.return_value = progress_instance
        main(["do something"])

    # Done path must call progress.done()
    progress_instance.done.assert_called_once()


# ---------------------------------------------------------------------------
# 12. _force_utf8_io reconfigures stdout/stderr to UTF-8 (L-3 fix)
# ---------------------------------------------------------------------------

def test_force_utf8_io_calls_reconfigure_with_utf8():
    """_force_utf8_io must call .reconfigure(encoding='utf-8', errors='replace')
    on stdout and stderr so non-ASCII characters (em-dash in --help text,
    JSON envelopes) survive the platform default code page."""
    fake_stdout = MagicMock(spec=["reconfigure"])
    fake_stderr = MagicMock(spec=["reconfigure"])
    with patch.object(sys, "stdout", fake_stdout), patch.object(sys, "stderr", fake_stderr):
        _force_utf8_io()
    fake_stdout.reconfigure.assert_called_once_with(encoding="utf-8", errors="replace")
    fake_stderr.reconfigure.assert_called_once_with(encoding="utf-8", errors="replace")


def test_force_utf8_io_is_safe_when_reconfigure_missing():
    """Streams without reconfigure() (older Pythons, redirected non-text
    streams) must be a silent no-op rather than raising AttributeError."""
    plain = object()  # no .reconfigure attribute
    with patch.object(sys, "stdout", plain), patch.object(sys, "stderr", plain):
        _force_utf8_io()  # must not raise


def test_force_utf8_io_swallows_reconfigure_errors():
    """If reconfigure() raises (closed stream, unsupported encoding) the
    helper drops the error silently to keep CLI startup robust."""
    failing = MagicMock(spec=["reconfigure"])
    failing.reconfigure.side_effect = ValueError("nope")
    with patch.object(sys, "stdout", failing), patch.object(sys, "stderr", failing):
        _force_utf8_io()  # must not raise
