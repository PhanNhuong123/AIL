"""Provider-swap end-to-end tests (Phase 15 task 15.3).

These tests prove the task-15.3 acceptance criteria without live API keys:

  1. Every registered ``provider:model`` prefix resolves to the correct
     provider class and exposes the expected ``name``.
  2. Malformed or unknown ``--model`` specs cause ``main()`` to return exit
     code 2 before any MCP connection is attempted.
  3. Missing API keys surface as a workflow ``status="error"`` carrying the
     expected environment-variable name (planner catches the lazy
     ``ProviderConfigError`` from ``_client()``).
  4. The workflow reaches ``status="done"`` regardless of which provider
     ``.name`` is wired in — proving the orchestrator is provider-agnostic.
  5. Two consecutive ``main()`` calls with different ``--model`` specs both
     succeed, proving provider swap needs only the flag (no restart).
  6. Default ``--model`` is ``anthropic:claude-sonnet-4-5``.
"""
from __future__ import annotations

from typing import Any
from unittest.mock import MagicMock, Mock, patch

import pytest

from ail_agent.__main__ import _DEFAULT_MODEL, main
from ail_agent.orchestrator import (
    build_workflow,
    clear_workflow_context,
    initial_state,
    set_workflow_context,
)
from ail_agent.providers.base import CompletionResult, LLMProvider, ToolSpec

try:
    from agents.tests.test_workflow_e2e_mocked import FakeProvider, FakeToolkit
except ImportError:
    from .test_workflow_e2e_mocked import FakeProvider, FakeToolkit  # type: ignore[no-redef]


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture(autouse=True)
def reset_context():
    clear_workflow_context()
    yield
    clear_workflow_context()


def _mock_mcp_and_workflow(mock_mcp_cls: Mock, mock_build: Mock, status: str = "done") -> None:
    """Wire MCPToolkit and build_workflow mocks so main() can complete without real IO."""
    toolkit_instance = MagicMock()
    mock_mcp_cls.return_value.__enter__ = Mock(return_value=toolkit_instance)
    mock_mcp_cls.return_value.__exit__ = Mock(return_value=False)
    graph = MagicMock()
    graph.invoke.return_value = {"status": status, "error": None}
    mock_build.return_value = graph


# ---------------------------------------------------------------------------
# A. Registry resolves every provider prefix to the expected class + name
# ---------------------------------------------------------------------------

_PROVIDER_CASES = [
    ("anthropic:claude-sonnet-4-5", "AnthropicProvider", "anthropic"),
    ("openai:gpt-4o", "OpenAIProvider", "openai"),
    ("deepseek:deepseek-chat", "DeepSeekProvider", "deepseek"),
    ("alibaba:qwen-max", "AlibabaProvider", "alibaba"),
    ("ollama:llama3.1", "OllamaProvider", "ollama"),
]


@pytest.mark.parametrize("spec, expected_class, expected_name", _PROVIDER_CASES)
def test_each_provider_prefix_resolves_to_correct_instance(
    spec: str, expected_class: str, expected_name: str
) -> None:
    """Every ``provider:model`` prefix must produce a provider whose class name
    and ``.name`` match. Construction is side-effect free so no env var is
    needed at registry resolution time."""
    from ail_agent.registry import get_provider

    provider, model = get_provider(spec)
    assert type(provider).__name__ == expected_class
    assert provider.name == expected_name
    assert model == spec.partition(":")[2]


def test_qwen_alias_resolves_to_alibaba_with_alibaba_name() -> None:
    """`qwen:...` is an alias for `alibaba:...` and must still report
    ``provider.name == "alibaba"`` so tool-call normalisation stays consistent."""
    from ail_agent.registry import get_provider

    provider, model = get_provider("qwen:qwen-turbo")
    assert type(provider).__name__ == "AlibabaProvider"
    assert provider.name == "alibaba"
    assert model == "qwen-turbo"


# ---------------------------------------------------------------------------
# B. Malformed / unknown spec → main() returns exit code 2 before MCP spin-up
# ---------------------------------------------------------------------------

def test_main_exits_2_for_unknown_provider_prefix() -> None:
    """``--model foo:bar`` must fail fast at ``get_provider`` and never reach
    the MCPToolkit context — so MCP does not need to be mocked."""
    result = main(["do something", "--model", "foo:bar"])
    assert result == 2


def test_main_exits_2_for_malformed_model_spec_without_colon() -> None:
    """A spec missing the ``:`` separator must be rejected with exit code 2."""
    result = main(["do something", "--model", "no-colon-here"])
    assert result == 2


def test_main_exits_2_for_empty_model_half() -> None:
    """Empty provider or model halves must be rejected with exit code 2."""
    assert main(["do something", "--model", ":gpt-4o"]) == 2
    assert main(["do something", "--model", "openai:"]) == 2


# ---------------------------------------------------------------------------
# C. Missing API key → workflow error carrying env-var name (exit 1)
# ---------------------------------------------------------------------------

_KEY_PROVIDER_CASES = [
    ("ail_agent.providers.anthropic", "AnthropicProvider", "ANTHROPIC_API_KEY", "anthropic"),
    ("ail_agent.providers.openai", "OpenAIProvider", "OPENAI_API_KEY", "openai"),
    ("ail_agent.providers.deepseek", "DeepSeekProvider", "DEEPSEEK_API_KEY", "deepseek"),
    ("ail_agent.providers.alibaba", "AlibabaProvider", "DASHSCOPE_API_KEY", "alibaba"),
]


@pytest.mark.parametrize(
    "module_path, class_name, env_var, provider_name", _KEY_PROVIDER_CASES
)
def test_missing_api_key_surfaces_env_var_in_workflow_error(
    module_path: str,
    class_name: str,
    env_var: str,
    provider_name: str,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """With the env var removed, running the workflow with the real provider
    must produce ``status="error"`` and an error message mentioning the env
    var name — so users see a clear, provider-specific hint.

    The planner's ``except AgentError`` branch catches the lazy
    ``ProviderConfigError`` raised by ``_client()``.
    """
    monkeypatch.delenv(env_var, raising=False)

    import importlib

    module = importlib.import_module(module_path)
    provider_cls = getattr(module, class_name)
    provider = provider_cls()

    # FakeToolkit is safe here: the planner fails before any tool call runs,
    # so status transitions to "error" immediately at the first node.
    set_workflow_context(
        provider=provider,
        model="unused",
        toolkit=FakeToolkit(),
        emit=lambda _s: None,
    )
    state = initial_state(task="trigger missing-key path")
    graph = build_workflow()
    final = graph.invoke(state, config={"recursion_limit": 50})

    assert final["status"] == "error"
    error = final["error"] or ""
    assert env_var in error, (
        f"expected env var {env_var!r} in error message, got {error!r}"
    )
    assert provider_name in error, (
        f"expected provider name {provider_name!r} in error message, got {error!r}"
    )


# ---------------------------------------------------------------------------
# D. Workflow reaches done regardless of provider.name (orchestrator-agnostic)
# ---------------------------------------------------------------------------

class _NamedFakeProvider(FakeProvider):
    """FakeProvider subclass that lets each parametrised case assert its own
    ``name``. The superclass returns canned plan JSON from ``complete()`` and
    ``complete_with_tools()`` so the workflow reaches ``done``."""

    name: str = "fake"


@pytest.mark.parametrize(
    "provider_name", ["anthropic", "openai", "deepseek", "alibaba", "ollama"]
)
def test_workflow_reaches_done_under_each_provider_name(provider_name: str) -> None:
    """Any provider ``.name`` must drive the workflow to ``done`` — proving the
    orchestrator, planner, and coder do not hard-code provider identity."""
    plan_steps: list[dict[str, Any]] = [
        {
            "pattern": "define",
            "intent": "Create a diagnostic node",
            "parent_id": "root",
            "label": "diagnostic",
        }
    ]
    provider = _NamedFakeProvider(plan_steps)
    provider.name = provider_name  # type: ignore[misc]

    set_workflow_context(
        provider=provider,
        model=f"{provider_name}:model",
        toolkit=FakeToolkit(),
        emit=lambda _s: None,
    )
    state = initial_state(task=f"swap task for {provider_name}")
    graph = build_workflow()
    final = graph.invoke(state, config={"recursion_limit": 200})

    assert final["status"] == "done", (
        f"provider={provider_name!r}: expected done, got {final['status']!r}: {final.get('error')!r}"
    )
    assert final["current_step"] == 1


# ---------------------------------------------------------------------------
# E. Provider swap needs only --model — two consecutive main() calls succeed
# ---------------------------------------------------------------------------

@patch("ail_agent.__main__.build_workflow")
@patch("ail_agent.__main__.set_workflow_context")
@patch("ail_agent.__main__.clear_workflow_context")
@patch("ail_agent.__main__.MCPToolkit")
@patch("ail_agent.__main__.get_provider")
def test_provider_swap_between_main_invocations_no_restart(
    mock_get_provider: Mock,
    mock_mcp_cls: Mock,
    _mock_clear: Mock,
    _mock_set_ctx: Mock,
    mock_build: Mock,
) -> None:
    """Two consecutive ``main()`` calls with different ``--model`` specs must
    both return 0 in the same Python process. Proves provider swap is purely
    a flag change — no config, env var, or restart is required."""
    mock_get_provider.return_value = (Mock(), "unused")
    _mock_mcp_and_workflow(mock_mcp_cls, mock_build, status="done")

    rc1 = main(["task one", "--model", "anthropic:claude-sonnet-4-5"])
    rc2 = main(["task two", "--model", "openai:gpt-4o"])

    assert rc1 == 0
    assert rc2 == 0
    assert mock_get_provider.call_count == 2
    assert mock_get_provider.call_args_list[0].args[0] == "anthropic:claude-sonnet-4-5"
    assert mock_get_provider.call_args_list[1].args[0] == "openai:gpt-4o"


# ---------------------------------------------------------------------------
# F. Default model
# ---------------------------------------------------------------------------

@patch("ail_agent.__main__.build_workflow")
@patch("ail_agent.__main__.set_workflow_context")
@patch("ail_agent.__main__.clear_workflow_context")
@patch("ail_agent.__main__.MCPToolkit")
@patch("ail_agent.__main__.get_provider")
def test_main_defaults_to_anthropic_claude_sonnet_4_5_when_model_flag_omitted(
    mock_get_provider: Mock,
    mock_mcp_cls: Mock,
    _mock_clear: Mock,
    _mock_set_ctx: Mock,
    mock_build: Mock,
) -> None:
    """Omitting ``--model`` must resolve to ``anthropic:claude-sonnet-4-5``,
    locking the default spec documented in CLAUDE.md and the reference spec."""
    mock_get_provider.return_value = (Mock(), "claude-sonnet-4-5")
    _mock_mcp_and_workflow(mock_mcp_cls, mock_build, status="done")

    rc = main(["do something"])

    assert rc == 0
    assert _DEFAULT_MODEL == "anthropic:claude-sonnet-4-5"
    assert mock_get_provider.call_args.args[0] == "anthropic:claude-sonnet-4-5"
