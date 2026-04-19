"""Tests for ail_agent.registry — model spec parsing and provider resolution."""
from __future__ import annotations

import pytest

from ail_agent.errors import ProviderConfigError
from ail_agent.registry import get_provider, parse_model_spec


def test_parse_model_spec_splits_prefix_and_model() -> None:
    prefix, model = parse_model_spec("openai:gpt-4o")
    assert prefix == "openai"
    assert model == "gpt-4o"


def test_parse_model_spec_rejects_missing_colon() -> None:
    with pytest.raises(ProviderConfigError) as exc_info:
        parse_model_spec("gpt-4o")
    assert "expected '<provider>:<model>'" in str(exc_info.value)


def test_parse_model_spec_rejects_empty_halves() -> None:
    with pytest.raises(ProviderConfigError):
        parse_model_spec(":gpt-4o")
    with pytest.raises(ProviderConfigError):
        parse_model_spec("openai:")


def test_parse_model_spec_lowercases_prefix() -> None:
    prefix, model = parse_model_spec("Anthropic:xyz")
    assert prefix == "anthropic"
    assert model == "xyz"


def test_get_provider_returns_tuple_of_instance_and_model(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("OPENAI_API_KEY", "test-key")
    provider, model = get_provider("openai:gpt-4o")
    assert model == "gpt-4o"
    assert provider.name == "openai"


def test_get_provider_unknown_prefix_raises_with_valid_list() -> None:
    with pytest.raises(ProviderConfigError) as exc_info:
        get_provider("foo:bar")
    msg = str(exc_info.value)
    assert "anthropic" in msg
    assert "openai" in msg
    assert "qwen" in msg


def test_get_provider_qwen_alias_resolves_to_alibaba_provider(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("DASHSCOPE_API_KEY", "test-key")
    provider, model = get_provider("qwen:qwen-max")
    assert model == "qwen-max"
    assert type(provider).__name__ == "AlibabaProvider"


def test_get_provider_missing_api_key_propagates_provider_config_error(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("ANTHROPIC_API_KEY", raising=False)
    # get_provider instantiates the class; _client() is only called on use.
    provider, _model = get_provider("anthropic:xxx")
    # Calling _client() triggers the env-var check
    with pytest.raises(ProviderConfigError) as exc_info:
        provider._client()  # type: ignore[attr-defined]
    assert "ANTHROPIC_API_KEY" in str(exc_info.value)
