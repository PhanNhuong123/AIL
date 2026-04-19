"""Tests for ail_agent.errors — AIL-G014x error hierarchy."""
from __future__ import annotations

import pytest

from ail_agent.errors import (
    AgentError,
    ProviderConfigError,
    ProviderError,
    RoutingError,
)


def test_agent_error_default_code_is_g0140() -> None:
    assert AgentError.code == "AIL-G0140"
    err = AgentError("something went wrong")
    assert err.code == "AIL-G0140"


def test_agent_error_message_includes_code() -> None:
    err = AgentError("hi")
    assert str(err).startswith("[AIL-G0140] hi")


def test_provider_error_includes_provider_name_in_message() -> None:
    err = ProviderError("boom", provider="anthropic")
    assert "anthropic: boom" in str(err)


def test_provider_error_preserves_cause_chain() -> None:
    cause = ValueError("x")
    err = ProviderError("failed", provider="openai", cause=cause)
    assert err.__cause__ is cause


def test_provider_config_error_is_agent_error() -> None:
    err = ProviderConfigError("bad config")
    assert isinstance(err, AgentError)


def test_routing_error_code_is_g0141() -> None:
    assert RoutingError.code == "AIL-G0141"
    err = RoutingError("cannot route")
    assert "[AIL-G0141]" in str(err)
    assert "cannot route" in str(err)
