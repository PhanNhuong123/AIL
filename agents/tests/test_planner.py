"""Tests for ail_agent.planner.run_planner (task 14.3)."""
from __future__ import annotations

import json
from typing import Any, Optional

import pytest

from ail_agent.errors import PlanError, ProviderError
from ail_agent.orchestrator import AILAgentState, initial_state
from ail_agent.planner import run_planner
from ail_agent.prompts import PLANNER_SYSTEM_PROMPT


# ---------------------------------------------------------------------------
# FakeProvider — satisfies the LLMProvider Protocol without network I/O
# ---------------------------------------------------------------------------

class FakeProvider:
    """Test double for LLMProvider.

    Pass a string to return it as the completion response.
    Pass an Exception instance to have it raised on the first complete() call.
    """

    name: str = "fake"

    def __init__(self, response: str | Exception) -> None:
        self._response = response
        self.calls: list[tuple[str, str, str]] = []

    def complete(self, system: str, user: str, *, model: str) -> str:
        self.calls.append((system, user, model))
        if isinstance(self._response, Exception):
            raise self._response
        return self._response

    def complete_with_tools(self, **kw: Any) -> Any:  # noqa: ANN401
        raise NotImplementedError


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_VALID_STEP_JSON: str = json.dumps(
    {
        "steps": [
            {
                "pattern": "do",
                "intent": "scaffold the module",
                "parent_id": "root",
            }
        ]
    }
)

_TWO_STEP_JSON: str = json.dumps(
    {
        "steps": [
            {
                "pattern": "define",
                "intent": "create the data model",
                "parent_id": "root",
                "label": "data_model",
            },
            {
                "pattern": "test",
                "intent": "write unit tests",
                "parent_id": "data_model",
            },
        ]
    }
)


def _base_state(**overrides: Any) -> AILAgentState:
    """Return a fresh initial state with optional field overrides."""
    state = initial_state(
        task="implement feature X",
        model_spec="anthropic:claude-sonnet-4-5",  # type: ignore[call-arg]
    )
    # initial_state does not accept model_spec; build manually if needed
    state.update(overrides)  # type: ignore[arg-type]
    return state


# ---------------------------------------------------------------------------
# 1. Happy path — valid single-step plan
# ---------------------------------------------------------------------------

def test_planner_happy_path() -> None:
    provider = FakeProvider(_VALID_STEP_JSON)
    state = initial_state(task="scaffold the module")

    result = run_planner(state, provider=provider, model="claude-sonnet-4-5")

    assert result["status"] == "code"
    assert isinstance(result["plan"], list)
    assert len(result["plan"]) == 1
    assert result["current_step"] == 0
    assert result["error"] is None


# ---------------------------------------------------------------------------
# 2. Prompt construction — task and context appear in the user message
# ---------------------------------------------------------------------------

def test_planner_passes_task_and_context_into_prompt() -> None:
    provider = FakeProvider(_VALID_STEP_JSON)
    state = initial_state(task="add caching layer")
    state["project_context"] = "existing Redis setup"  # type: ignore[typeddict-unknown-key]

    run_planner(state, provider=provider, model="gpt-4o")

    assert len(provider.calls) == 1
    system_msg, user_msg, _ = provider.calls[0]

    assert system_msg == PLANNER_SYSTEM_PROMPT
    assert "add caching layer" in user_msg
    assert "existing Redis setup" in user_msg


# ---------------------------------------------------------------------------
# 3. Missing project_context defaults to "(none)"
# ---------------------------------------------------------------------------

def test_planner_uses_default_context_when_missing() -> None:
    provider = FakeProvider(_VALID_STEP_JSON)
    state = initial_state(task="do something")
    # Ensure project_context key is absent
    state.pop("project_context", None)  # type: ignore[misc]

    run_planner(state, provider=provider, model="gpt-4o")

    _, user_msg, _ = provider.calls[0]
    assert "(none)" in user_msg


# ---------------------------------------------------------------------------
# 4. ProviderError is caught and reflected in state
# ---------------------------------------------------------------------------

def test_planner_handles_provider_error() -> None:
    exc = ProviderError("rate limit", provider="anthropic")
    provider = FakeProvider(exc)
    state = initial_state(task="do something")

    result = run_planner(state, provider=provider, model="claude-sonnet-4-5")

    assert result["status"] == "error"
    assert result["error"] is not None
    assert "AIL-G0140" in result["error"]
    assert "rate limit" in result["error"]


# ---------------------------------------------------------------------------
# 5. Invalid JSON response → PlanError with AIL-G0144
# ---------------------------------------------------------------------------

def test_planner_handles_invalid_json() -> None:
    provider = FakeProvider("not json")
    state = initial_state(task="do something")

    result = run_planner(state, provider=provider, model="claude-sonnet-4-5")

    assert result["status"] == "error"
    assert result["error"] is not None
    assert "AIL-G0144" in result["error"]
    assert "invalid JSON" in result["error"]


# ---------------------------------------------------------------------------
# 6. Unknown pattern in step → error mentions "unknown pattern"
# ---------------------------------------------------------------------------

def test_planner_handles_invalid_pattern() -> None:
    bad_json = json.dumps(
        {
            "steps": [
                {
                    "pattern": "while",
                    "intent": "loop forever",
                    "parent_id": "root",
                }
            ]
        }
    )
    provider = FakeProvider(bad_json)
    state = initial_state(task="do something")

    result = run_planner(state, provider=provider, model="claude-sonnet-4-5")

    assert result["status"] == "error"
    assert result["error"] is not None
    assert "unknown pattern" in result["error"]


# ---------------------------------------------------------------------------
# 7. Missing required field "intent" → error mentions field name
# ---------------------------------------------------------------------------

def test_planner_handles_missing_required_field() -> None:
    bad_json = json.dumps(
        {
            "steps": [
                {
                    "pattern": "do",
                    # "intent" deliberately omitted
                    "parent_id": "root",
                }
            ]
        }
    )
    provider = FakeProvider(bad_json)
    state = initial_state(task="do something")

    result = run_planner(state, provider=provider, model="claude-sonnet-4-5")

    assert result["status"] == "error"
    assert result["error"] is not None
    assert "required field missing" in result["error"]
    assert "intent" in result["error"]


# ---------------------------------------------------------------------------
# 8. Input state dict must not be mutated
# ---------------------------------------------------------------------------

def test_planner_does_not_mutate_input_state() -> None:
    provider = FakeProvider(_VALID_STEP_JSON)
    state = initial_state(task="scaffold")
    original_id = id(state)
    snapshot: dict[str, Any] = dict(state)

    result = run_planner(state, provider=provider, model="claude-sonnet-4-5")

    # The returned state is a new dict
    assert id(result) != original_id
    # The input state is unmodified
    for key, original_value in snapshot.items():
        assert state[key] == original_value, (  # type: ignore[literal-required]
            f"Input state key {key!r} was mutated: {state[key]!r} != {original_value!r}"  # type: ignore[literal-required]
        )


# ---------------------------------------------------------------------------
# 9. The model argument is forwarded to the provider
# ---------------------------------------------------------------------------

def test_planner_uses_provided_model() -> None:
    provider = FakeProvider(_VALID_STEP_JSON)
    state = initial_state(task="do something")

    run_planner(state, provider=provider, model="claude-sonnet-4-5")

    assert len(provider.calls) == 1
    _, _, used_model = provider.calls[0]
    assert used_model == "claude-sonnet-4-5"


# ---------------------------------------------------------------------------
# 10. Success path clears a pre-existing error
# ---------------------------------------------------------------------------

def test_planner_clears_previous_error() -> None:
    provider = FakeProvider(_VALID_STEP_JSON)
    state = initial_state(task="retry after fix")
    state["error"] = "previous run exploded"
    state["status"] = "plan"

    result = run_planner(state, provider=provider, model="claude-sonnet-4-5")

    assert result["status"] == "code"
    # error field must be cleared (None, same as initial_state default)
    assert result["error"] is None


# ---------------------------------------------------------------------------
# Bonus: AgentError subclass (non-ProviderError) is also caught
# ---------------------------------------------------------------------------

def test_planner_handles_generic_agent_error() -> None:
    from ail_agent.errors import AgentError

    exc = AgentError("unexpected agent failure")
    provider = FakeProvider(exc)
    state = initial_state(task="do something")

    result = run_planner(state, provider=provider, model="claude-sonnet-4-5")

    assert result["status"] == "error"
    assert result["error"] is not None
    assert "AIL-G0140" in result["error"]


# ---------------------------------------------------------------------------
# Bonus: Multi-step plan populates plan list correctly
# ---------------------------------------------------------------------------

def test_planner_multi_step_plan() -> None:
    provider = FakeProvider(_TWO_STEP_JSON)
    state = initial_state(task="build feature")

    result = run_planner(state, provider=provider, model="claude-sonnet-4-5")

    assert result["status"] == "code"
    assert isinstance(result["plan"], list)
    assert len(result["plan"]) == 2
    assert result["current_step"] == 0
