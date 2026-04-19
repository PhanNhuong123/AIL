"""Tests for ail_agent.plan_format — parse_plan() validation and happy paths."""
from __future__ import annotations

import json

import pytest

from ail_agent.errors import PlanError
from ail_agent.plan_format import VALID_PATTERNS, PlanStep, parse_plan


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_MINIMAL_STEP: dict = {
    "pattern": "do",
    "intent": "Add input validation",
    "parent_id": "root",
}

_FULL_STEP: dict = {
    "pattern": "fix",
    "intent": "Fix the transfer overflow",
    "parent_id": "root",
    "expression": "transfer_money(amount)",
    "label": "fix_overflow",
    "contracts": [
        {"kind": "before", "expression": "amount > 0"},
        {"kind": "after", "expression": "balance >= 0"},
    ],
    "metadata": {"name": "fix_overflow_node", "priority": 1},
}


def _wrap(*steps: dict) -> dict:
    """Return a {"steps": [...]} wrapper."""
    return {"steps": list(steps)}


def _json(*steps: dict) -> str:
    return json.dumps(_wrap(*steps))


# ---------------------------------------------------------------------------
# Happy paths
# ---------------------------------------------------------------------------


def test_parse_plan_happy_path_minimal_step() -> None:
    result = parse_plan(_json(_MINIMAL_STEP))
    assert isinstance(result, list)
    assert len(result) == 1
    step = result[0]
    assert step["pattern"] == "do"
    assert step["intent"] == "Add input validation"
    assert step["parent_id"] == "root"


def test_parse_plan_happy_path_full_step() -> None:
    result = parse_plan(_json(_FULL_STEP))
    assert len(result) == 1
    step = result[0]
    assert step["pattern"] == "fix"
    assert step["label"] == "fix_overflow"
    assert step["expression"] == "transfer_money(amount)"
    assert step["contracts"][0]["kind"] == "before"
    assert step["metadata"]["name"] == "fix_overflow_node"


def test_parse_plan_accepts_dict_input() -> None:
    """Pre-decoded dict skips JSON parse entirely."""
    data = _wrap(_MINIMAL_STEP)
    result = parse_plan(data)
    assert len(result) == 1
    assert result[0]["pattern"] == "do"


def test_parse_plan_returns_list_of_dicts() -> None:
    steps = [_MINIMAL_STEP, _FULL_STEP]
    result = parse_plan(_wrap(*steps))
    assert isinstance(result, list)
    assert len(result) == 2
    for item in result:
        assert isinstance(item, dict)


# ---------------------------------------------------------------------------
# Top-level wrapper validation errors
# ---------------------------------------------------------------------------


def test_parse_plan_invalid_json() -> None:
    with pytest.raises(PlanError) as ei:
        parse_plan("not json {{{{")
    err = ei.value
    assert err.field_name == "response"
    assert err.step_index is None
    assert "invalid JSON" in str(err)
    # cause must be the original JSONDecodeError
    assert err.__cause__ is not None


def test_parse_plan_top_level_not_dict() -> None:
    """A JSON array at the top level is not a valid plan object."""
    with pytest.raises(PlanError) as ei:
        parse_plan("[]")
    err = ei.value
    assert err.field_name == "response"
    assert err.step_index is None
    assert "not a JSON object" in str(err)


def test_parse_plan_missing_steps_key() -> None:
    with pytest.raises(PlanError) as ei:
        parse_plan({})
    err = ei.value
    assert err.field_name == "steps"
    assert err.step_index is None
    assert "required field missing" in str(err)


def test_parse_plan_steps_not_list() -> None:
    with pytest.raises(PlanError) as ei:
        parse_plan({"steps": "x"})
    err = ei.value
    assert err.field_name == "steps"
    assert "steps must be a list" in str(err)


def test_parse_plan_empty_steps() -> None:
    with pytest.raises(PlanError) as ei:
        parse_plan({"steps": []})
    err = ei.value
    assert err.field_name == "steps"
    assert "at least one step" in str(err)


# ---------------------------------------------------------------------------
# Per-step structural errors
# ---------------------------------------------------------------------------


def test_parse_plan_step_not_dict() -> None:
    with pytest.raises(PlanError) as ei:
        parse_plan({"steps": ["x"]})
    err = ei.value
    assert err.step_index == 0
    assert "step must be an object" in str(err)


def test_parse_plan_missing_required_field_pattern() -> None:
    step = {"intent": "Do something", "parent_id": "root"}
    with pytest.raises(PlanError) as ei:
        parse_plan(_wrap(step))
    err = ei.value
    assert err.step_index == 0
    assert err.field_name == "pattern"
    assert "required field missing" in str(err)


def test_parse_plan_missing_required_field_intent() -> None:
    """Tests at step_index=1 to exercise non-zero index reporting."""
    valid_step = _MINIMAL_STEP.copy()
    step_missing_intent = {"pattern": "do", "parent_id": "root"}
    with pytest.raises(PlanError) as ei:
        parse_plan(_wrap(valid_step, step_missing_intent))
    err = ei.value
    assert err.step_index == 1
    assert err.field_name == "intent"
    assert "required field missing" in str(err)


def test_parse_plan_missing_required_field_parent_id() -> None:
    step = {"pattern": "do", "intent": "Do something"}
    with pytest.raises(PlanError) as ei:
        parse_plan(_wrap(step))
    err = ei.value
    assert err.step_index == 0
    assert err.field_name == "parent_id"
    assert "required field missing" in str(err)


# ---------------------------------------------------------------------------
# pattern-specific validation
# ---------------------------------------------------------------------------


def test_parse_plan_unknown_pattern() -> None:
    step = {"pattern": "while", "intent": "Loop forever", "parent_id": "root"}
    with pytest.raises(PlanError) as ei:
        parse_plan(_wrap(step))
    err = ei.value
    assert err.step_index == 0
    assert err.field_name == "pattern"
    assert "while" in str(err)
    # All valid patterns must appear in the message, sorted
    for p in sorted(VALID_PATTERNS):
        assert p in str(err)


def test_parse_plan_pattern_wrong_type() -> None:
    step = {"pattern": 123, "intent": "Do something", "parent_id": "root"}
    with pytest.raises(PlanError) as ei:
        parse_plan(_wrap(step))
    err = ei.value
    assert err.step_index == 0
    assert err.field_name == "pattern"
    assert "non-empty string" in str(err)


def test_parse_plan_intent_must_be_nonempty_string() -> None:
    step = {"pattern": "do", "intent": "   ", "parent_id": "root"}
    with pytest.raises(PlanError) as ei:
        parse_plan(_wrap(step))
    err = ei.value
    assert err.step_index == 0
    assert err.field_name == "intent"
    assert "non-empty string" in str(err)


# ---------------------------------------------------------------------------
# Optional field validation
# ---------------------------------------------------------------------------


def test_parse_plan_label_empty_string_rejected() -> None:
    step = {**_MINIMAL_STEP, "label": ""}
    with pytest.raises(PlanError) as ei:
        parse_plan(_wrap(step))
    err = ei.value
    assert err.step_index == 0
    assert err.field_name == "label"
    assert "non-empty string" in str(err)


def test_parse_plan_metadata_wrong_type() -> None:
    step = {**_MINIMAL_STEP, "metadata": "x"}
    with pytest.raises(PlanError) as ei:
        parse_plan(_wrap(step))
    err = ei.value
    assert err.step_index == 0
    assert err.field_name == "metadata"
    assert "metadata must be a dict" in str(err)


def test_parse_plan_invalid_contract_kind() -> None:
    step = {
        **_MINIMAL_STEP,
        "contracts": [{"kind": "keep", "expression": "x > 0"}],
    }
    with pytest.raises(PlanError) as ei:
        parse_plan(_wrap(step))
    err = ei.value
    assert err.step_index == 0
    assert err.field_name == "contracts[0].kind"
    assert "keep" in str(err)


# ---------------------------------------------------------------------------
# Immutability
# ---------------------------------------------------------------------------


def test_valid_patterns_immutable() -> None:
    assert isinstance(VALID_PATTERNS, frozenset)
    with pytest.raises((AttributeError, TypeError)):
        VALID_PATTERNS.add("while")  # type: ignore[attr-defined]
