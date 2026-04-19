"""Tests for all provider adapters — uses mocks, no live credentials required."""
from __future__ import annotations

import json
from typing import Any
from unittest.mock import MagicMock, Mock, patch

import pytest

from ail_agent.errors import ProviderConfigError
from ail_agent.providers.base import LLMProvider


# ===========================================================================
# Anthropic
# ===========================================================================


def test_anthropic_missing_env_var_raises_provider_config_error(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("ANTHROPIC_API_KEY", raising=False)
    from ail_agent.providers.anthropic import AnthropicProvider

    with pytest.raises(ProviderConfigError) as exc_info:
        AnthropicProvider()._client()
    assert "ANTHROPIC_API_KEY" in str(exc_info.value)


def test_anthropic_complete_returns_concatenated_text_blocks(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("ANTHROPIC_API_KEY", "test-key")

    block1 = Mock(type="text", text="hi ")
    block2 = Mock(type="text", text="there")
    mock_resp = Mock(content=[block1, block2])

    mock_client = MagicMock()
    mock_client.messages.create.return_value = mock_resp

    with patch("ail_agent.providers.anthropic.anthropic.Anthropic", return_value=mock_client):
        from ail_agent.providers.anthropic import AnthropicProvider

        provider = AnthropicProvider()
        result = provider.complete("sys", "user", model="claude-test")

    assert result == "hi there"


def test_anthropic_complete_with_tools_normalizes_tool_use_blocks(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("ANTHROPIC_API_KEY", "test-key")

    tool_block = Mock(type="tool_use", id="tool_1", input={"a": 1})
    tool_block.name = "ail.write"  # Must set separately; Mock(name=...) sets mock name
    mock_resp = Mock(content=[tool_block])

    mock_client = MagicMock()
    mock_client.messages.create.return_value = mock_resp

    with patch("ail_agent.providers.anthropic.anthropic.Anthropic", return_value=mock_client):
        from ail_agent.providers.anthropic import AnthropicProvider

        provider = AnthropicProvider()
        tool_spec: Any = {"name": "ail.write", "description": "write", "input_schema": {"type": "object"}}
        result = provider.complete_with_tools("sys", "user", model="claude-test", tools=[tool_spec])

    assert len(result["tool_calls"]) == 1
    tc = result["tool_calls"][0]
    assert tc["id"] == "tool_1"
    assert tc["name"] == "ail.write"
    assert tc["arguments"] == {"a": 1}


def test_anthropic_retry_exhaustion_raises_provider_error(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("ANTHROPIC_API_KEY", "test-key")
    # Patch time.sleep in the retry module to avoid real waits
    monkeypatch.setattr("ail_agent.providers._retry.time.sleep", lambda d: None)

    import anthropic as anthropic_sdk

    mock_client = MagicMock()
    # Simulate a connection error that should be retried
    mock_client.messages.create.side_effect = anthropic_sdk.APIConnectionError(
        request=MagicMock()
    )

    with patch("ail_agent.providers.anthropic.anthropic.Anthropic", return_value=mock_client):
        from ail_agent.errors import ProviderError
        from ail_agent.providers.anthropic import AnthropicProvider

        provider = AnthropicProvider()
        with pytest.raises(ProviderError):
            provider.complete("sys", "user", model="claude-test")


# ===========================================================================
# OpenAI
# ===========================================================================


def test_openai_missing_env_var_raises_provider_config_error(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("OPENAI_API_KEY", raising=False)
    from ail_agent.providers.openai import OpenAIProvider

    with pytest.raises(ProviderConfigError) as exc_info:
        OpenAIProvider()._client()
    assert "OPENAI_API_KEY" in str(exc_info.value)


def test_openai_complete_returns_message_content(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("OPENAI_API_KEY", "test-key")

    mock_resp = Mock(choices=[Mock(message=Mock(content="ok"))])
    mock_client = MagicMock()
    mock_client.chat.completions.create.return_value = mock_resp

    with patch("ail_agent.providers.openai.openai.OpenAI", return_value=mock_client):
        from ail_agent.providers.openai import OpenAIProvider

        provider = OpenAIProvider()
        result = provider.complete("sys", "user", model="gpt-4o")

    assert result == "ok"


def test_openai_complete_with_tools_parses_json_string_arguments(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("OPENAI_API_KEY", "test-key")

    fn_mock = Mock(arguments='{"a": 1}')
    fn_mock.name = "ail.write"  # Must set separately; Mock(name=...) sets mock name
    tc_mock = Mock(id="c1", function=fn_mock)
    mock_resp = Mock(choices=[Mock(message=Mock(content=None, tool_calls=[tc_mock]))])
    mock_client = MagicMock()
    mock_client.chat.completions.create.return_value = mock_resp

    with patch("ail_agent.providers.openai.openai.OpenAI", return_value=mock_client):
        from ail_agent.providers.openai import OpenAIProvider

        provider = OpenAIProvider()
        tool_spec: Any = {"name": "ail.write", "description": "write", "input_schema": {"type": "object"}}
        result = provider.complete_with_tools("sys", "user", model="gpt-4o", tools=[tool_spec])

    assert len(result["tool_calls"]) == 1
    tc = result["tool_calls"][0]
    assert tc["id"] == "c1"
    assert tc["name"] == "ail.write"
    assert tc["arguments"] == {"a": 1}


# ===========================================================================
# DeepSeek
# ===========================================================================


def test_deepseek_missing_env_var_raises_provider_config_error(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("DEEPSEEK_API_KEY", raising=False)
    from ail_agent.providers.deepseek import DeepSeekProvider

    with pytest.raises(ProviderConfigError) as exc_info:
        DeepSeekProvider()._client()
    assert "DEEPSEEK_API_KEY" in str(exc_info.value)


def test_deepseek_uses_correct_base_url(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("DEEPSEEK_API_KEY", "test-key")

    with patch("ail_agent.providers.deepseek.openai.OpenAI") as mock_openai_cls:
        mock_openai_cls.return_value = MagicMock()
        from ail_agent.providers.deepseek import DeepSeekProvider

        DeepSeekProvider()._client()

    call_kwargs = mock_openai_cls.call_args[1]
    assert call_kwargs["base_url"] == "https://api.deepseek.com"


def test_deepseek_reasoner_falls_back_to_prompt_mode(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """deepseek-reasoner is in NON_TOOL_MODELS, so it must use prompt fallback."""
    monkeypatch.setenv("DEEPSEEK_API_KEY", "test-key")

    fence_response = (
        "Sure!\n```json\n"
        '{"tool": "ail.write", "arguments": {"x": 1}}\n'
        "```"
    )
    mock_resp = Mock(choices=[Mock(message=Mock(content=fence_response))])
    mock_client = MagicMock()
    mock_client.chat.completions.create.return_value = mock_resp

    with patch("ail_agent.providers.deepseek.openai.OpenAI", return_value=mock_client):
        from ail_agent.providers.deepseek import DeepSeekProvider

        provider = DeepSeekProvider()
        tool_spec: Any = {"name": "ail.write", "description": "write", "input_schema": {"type": "object"}}
        result = provider.complete_with_tools(
            "sys", "user", model="deepseek-reasoner", tools=[tool_spec]
        )

    # In prompt-fallback mode, the call to create() must NOT have a ``tools=`` kwarg
    call_kwargs = mock_client.chat.completions.create.call_args[1]
    assert "tools" not in call_kwargs
    # The augmented system prompt must mention the available tools
    system_sent = call_kwargs["messages"][0]["content"]
    assert "Available tools" in system_sent

    assert len(result["tool_calls"]) == 1
    assert result["tool_calls"][0]["name"] == "ail.write"
    assert result["tool_calls"][0]["arguments"] == {"x": 1}


# ===========================================================================
# Alibaba
# ===========================================================================


def test_alibaba_missing_env_var_raises_provider_config_error(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("DASHSCOPE_API_KEY", raising=False)
    from ail_agent.providers.alibaba import AlibabaProvider

    with pytest.raises(ProviderConfigError) as exc_info:
        AlibabaProvider()._client()
    assert "DASHSCOPE_API_KEY" in str(exc_info.value)


def test_alibaba_uses_dashscope_base_url(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("DASHSCOPE_API_KEY", "test-key")

    with patch("ail_agent.providers.alibaba.openai.OpenAI") as mock_openai_cls:
        mock_openai_cls.return_value = MagicMock()
        from ail_agent.providers.alibaba import AlibabaProvider

        AlibabaProvider()._client()

    call_kwargs = mock_openai_cls.call_args[1]
    assert call_kwargs["base_url"] == "https://dashscope-intl.aliyuncs.com/compatible-mode/v1"


def test_alibaba_qwen_turbo_falls_back_to_prompt_mode(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """qwen-turbo is in NON_TOOL_MODELS, so it must use prompt fallback."""
    monkeypatch.setenv("DASHSCOPE_API_KEY", "test-key")

    fence_response = (
        "Here:\n```json\n"
        '{"tool": "ail.write", "arguments": {"y": 2}}\n'
        "```"
    )
    mock_resp = Mock(choices=[Mock(message=Mock(content=fence_response))])
    mock_client = MagicMock()
    mock_client.chat.completions.create.return_value = mock_resp

    with patch("ail_agent.providers.alibaba.openai.OpenAI", return_value=mock_client):
        from ail_agent.providers.alibaba import AlibabaProvider

        provider = AlibabaProvider()
        tool_spec: Any = {"name": "ail.write", "description": "write", "input_schema": {"type": "object"}}
        result = provider.complete_with_tools(
            "sys", "user", model="qwen-turbo", tools=[tool_spec]
        )

    call_kwargs = mock_client.chat.completions.create.call_args[1]
    assert "tools" not in call_kwargs
    system_sent = call_kwargs["messages"][0]["content"]
    assert "Available tools" in system_sent

    assert len(result["tool_calls"]) == 1
    assert result["tool_calls"][0]["name"] == "ail.write"
    assert result["tool_calls"][0]["arguments"] == {"y": 2}


# ===========================================================================
# Ollama
# ===========================================================================


def _make_httpx_response(data: Any) -> Mock:
    """Helper: create a mock httpx response with JSON body."""
    mock_resp = Mock()
    mock_resp.json.return_value = data
    mock_resp.raise_for_status.return_value = None
    return mock_resp


def _make_httpx_client(post_response: Any) -> MagicMock:
    """Helper: create a mock httpx.Client context manager."""
    mock_client = MagicMock()
    mock_client.__enter__ = Mock(return_value=mock_client)
    mock_client.__exit__ = Mock(return_value=False)
    mock_client.post.return_value = post_response
    return mock_client


def test_ollama_uses_env_base_url_or_default(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    from ail_agent.providers.ollama import OllamaProvider

    # With env var set
    monkeypatch.setenv("OLLAMA_BASE_URL", "http://myhost:8080")
    assert OllamaProvider()._base_url() == "http://myhost:8080"

    # Without env var
    monkeypatch.delenv("OLLAMA_BASE_URL", raising=False)
    assert OllamaProvider()._base_url() == "http://localhost:11434"


def test_ollama_complete_posts_to_api_chat(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("OLLAMA_BASE_URL", raising=False)

    data = {"message": {"content": "hello world", "tool_calls": []}}
    mock_resp = _make_httpx_response(data)
    mock_client = _make_httpx_client(mock_resp)

    with patch("ail_agent.providers.ollama.httpx.Client", return_value=mock_client):
        from ail_agent.providers.ollama import OllamaProvider

        provider = OllamaProvider()
        result = provider.complete("sys", "user", model="llama3.1")

    assert result == "hello world"
    call_args = mock_client.post.call_args
    assert call_args[0][0].endswith("/api/chat")
    body = call_args[1]["json"]
    assert body["model"] == "llama3.1"
    assert body["stream"] is False


def test_ollama_llama31_uses_native_tools_path(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("OLLAMA_BASE_URL", raising=False)

    data = {
        "message": {
            "content": None,
            "tool_calls": [
                {"id": "t1", "function": {"name": "ail.write", "arguments": {"x": 1}}}
            ],
        }
    }
    mock_resp = _make_httpx_response(data)
    mock_client = _make_httpx_client(mock_resp)

    with patch("ail_agent.providers.ollama.httpx.Client", return_value=mock_client):
        from ail_agent.providers.ollama import OllamaProvider

        provider = OllamaProvider()
        tool_spec: Any = {"name": "ail.write", "description": "write", "input_schema": {"type": "object"}}
        result = provider.complete_with_tools("sys", "user", model="llama3.1", tools=[tool_spec])

    # Native path: ``tools=`` must be in the posted JSON body
    body = mock_client.post.call_args[1]["json"]
    assert "tools" in body

    assert len(result["tool_calls"]) == 1
    assert result["tool_calls"][0]["name"] == "ail.write"
    assert result["tool_calls"][0]["arguments"] == {"x": 1}


def test_ollama_unknown_model_uses_prompt_fallback_and_parses_json_fence(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("OLLAMA_BASE_URL", raising=False)

    fence_body = (
        "Let me help:\n"
        "```json\n"
        '{"tool": "ail.write", "arguments": {"x": 1}}\n'
        "```"
    )
    data = {"message": {"content": fence_body}}
    mock_resp = _make_httpx_response(data)
    mock_client = _make_httpx_client(mock_resp)

    with patch("ail_agent.providers.ollama.httpx.Client", return_value=mock_client):
        from ail_agent.providers.ollama import OllamaProvider

        provider = OllamaProvider()
        tool_spec: Any = {"name": "ail.write", "description": "write", "input_schema": {"type": "object"}}
        result = provider.complete_with_tools(
            "sys", "user", model="some-other-model", tools=[tool_spec]
        )

    # Prompt fallback: no ``tools=`` in the posted JSON body
    body = mock_client.post.call_args[1]["json"]
    assert "tools" not in body

    assert len(result["tool_calls"]) == 1
    assert result["tool_calls"][0] == {
        "id": "ollama-fallback-0",
        "name": "ail.write",
        "arguments": {"x": 1},
    }


# ===========================================================================
# Protocol conformance
# ===========================================================================


@pytest.mark.parametrize(
    "provider_cls_path",
    [
        "ail_agent.providers.anthropic.AnthropicProvider",
        "ail_agent.providers.openai.OpenAIProvider",
        "ail_agent.providers.deepseek.DeepSeekProvider",
        "ail_agent.providers.alibaba.AlibabaProvider",
        "ail_agent.providers.ollama.OllamaProvider",
    ],
)
def test_all_providers_satisfy_llm_provider_protocol(provider_cls_path: str) -> None:
    """Every provider must satisfy the LLMProvider Protocol at runtime.

    Constructor is safe because _client() is lazy (no env read at __init__).
    """
    module_path, cls_name = provider_cls_path.rsplit(".", 1)
    import importlib

    module = importlib.import_module(module_path)
    cls = getattr(module, cls_name)
    instance = cls()
    assert isinstance(instance, LLMProvider), (
        f"{cls_name} does not satisfy the LLMProvider Protocol"
    )
