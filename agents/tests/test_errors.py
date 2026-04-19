"""Tests for ail_agent.errors — AIL-G014x error hierarchy."""
from __future__ import annotations

import pytest

from ail_agent.errors import (
    AgentError,
    MCPConnectionError,
    PlanError,
    ProviderConfigError,
    ProviderError,
    RoutingError,
    StepBudgetError,
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


# ---------------------------------------------------------------------------
# StepBudgetError (AIL-G0143)
# ---------------------------------------------------------------------------


def test_step_budget_error_code() -> None:
    err = StepBudgetError("step budget exceeded")
    assert err.code == "AIL-G0143"
    assert str(err).startswith("[AIL-G0143] ")


def test_step_budget_error_is_agent_error() -> None:
    err = StepBudgetError("step budget exceeded")
    assert isinstance(err, AgentError)


# ---------------------------------------------------------------------------
# PlanError (AIL-G0144)
# ---------------------------------------------------------------------------


def test_plan_error_with_step_and_field() -> None:
    err = PlanError("required field missing", step_index=2, field_name="pattern")
    assert str(err) == "[AIL-G0144] step 2 field 'pattern': required field missing"
    assert err.step_index == 2
    assert err.field_name == "pattern"


def test_plan_error_with_only_step_index() -> None:
    err = PlanError("invalid pattern", step_index=0)
    assert str(err) == "[AIL-G0144] step 0: invalid pattern"


def test_plan_error_with_only_field_name() -> None:
    err = PlanError("invalid JSON: ...", field_name="response")
    assert str(err) == "[AIL-G0144] step None field 'response': invalid JSON: ..."


def test_plan_error_with_neither() -> None:
    err = PlanError("generic plan failure")
    assert str(err) == "[AIL-G0144] generic plan failure"
    assert err.step_index is None
    assert err.field_name is None


def test_plan_error_preserves_cause() -> None:
    cause = ValueError("bad JSON")
    err = PlanError("invalid JSON: ...", field_name="response", cause=cause)
    assert err.__cause__ is cause


def test_plan_error_is_agent_error() -> None:
    err = PlanError("something wrong with plan")
    assert isinstance(err, AgentError)


# ---------------------------------------------------------------------------
# MCPConnectionError (AIL-G0145)
# ---------------------------------------------------------------------------


def test_mcp_connection_error_code() -> None:
    err = MCPConnectionError("cannot connect to MCP server")
    assert err.code == "AIL-G0145"
    assert str(err).startswith("[AIL-G0145] ")


def test_mcp_connection_error_port_attr() -> None:
    err = MCPConnectionError("connection refused on port 7777", port=7777)
    assert err.port == 7777


def test_mcp_connection_error_default_port_none() -> None:
    err = MCPConnectionError("msg")
    assert err.port is None


def test_mcp_connection_error_is_agent_error() -> None:
    err = MCPConnectionError("cannot connect")
    assert isinstance(err, AgentError)
