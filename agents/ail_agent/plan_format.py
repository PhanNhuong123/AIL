"""Pure parser for planner LLM output. No I/O, no provider calls."""
from __future__ import annotations

import json
from typing import Any, NotRequired, TypedDict

from ail_agent.errors import PlanError

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

VALID_PATTERNS: frozenset[str] = frozenset(
    {
        "always",
        "check",
        "define",
        "describe",
        "do",
        "explain",
        "fix",
        "let",
        "raise",
        "set",
        "test",
        "use",
    }
)

# Contract kinds that exist in crates/ail-graph/src/types/contract.rs.
# The enum has exactly three variants: Before, After, Always (snake_case via
# serde).  "keep" from the original Phase-11/12 spec draft was never added to
# the Rust enum and is therefore intentionally excluded here.
_VALID_CONTRACT_KINDS: frozenset[str] = frozenset({"before", "after", "always"})


# ---------------------------------------------------------------------------
# TypedDicts
# ---------------------------------------------------------------------------


class ContractDict(TypedDict, total=False):
    kind: str        # required at runtime; total=False to allow partial during parse
    expression: str


class PlanStep(TypedDict, total=False):
    pattern: str                    # required: must be in VALID_PATTERNS
    intent: str                     # required: non-empty
    parent_id: str                  # required: UUID | "root" | label
    expression: str                 # optional
    label: str                      # optional explicit identifier
    contracts: list[ContractDict]   # optional
    metadata: dict[str, Any]        # optional shallow-merge into NodeMetadata


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def parse_plan(raw: str | dict[str, Any]) -> list[PlanStep]:
    """Parse a planner response into validated PlanSteps.

    Accepts either:
      - a JSON string (raises PlanError(field_name='response') if not valid
        JSON), OR
      - a pre-decoded dict (skips JSON decoding).

    Required top-level shape: {"steps": [...]}.
    Each step is validated; the FIRST validation failure raises PlanError with
    the offending step_index and field_name.  Returns the typed list on success.
    """
    data = _decode(raw)
    steps_raw = _validate_wrapper(data)
    result: list[PlanStep] = []
    for i, step in enumerate(steps_raw):
        result.append(_validate_step(step, i))
    return result


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------


def _decode(raw: str | dict[str, Any]) -> dict[str, Any]:
    """Decode *raw* to a dict, raising PlanError on any problem."""
    if isinstance(raw, str):
        try:
            decoded = json.loads(raw)
        except json.JSONDecodeError as exc:
            raise PlanError(
                f"invalid JSON: {exc.msg}",
                field_name="response",
                cause=exc,
            ) from exc
        if not isinstance(decoded, dict):
            raise PlanError(
                "response is not a JSON object",
                field_name="response",
            )
        return decoded  # type: ignore[return-value]

    if not isinstance(raw, dict):
        raise PlanError(
            "response is not a JSON object",
            field_name="response",
        )
    return raw  # type: ignore[return-value]


def _validate_wrapper(data: dict[str, Any]) -> list[Any]:
    """Validate the top-level {"steps": [...]} shape and return the list."""
    if "steps" not in data:
        raise PlanError("required field missing", field_name="steps")

    steps = data["steps"]
    if not isinstance(steps, list):
        raise PlanError("steps must be a list", field_name="steps")

    if len(steps) == 0:
        raise PlanError("plan must contain at least one step", field_name="steps")

    return steps  # type: ignore[return-value]


def _validate_step(step: Any, i: int) -> PlanStep:
    """Validate a single step dict at index *i* and return it as PlanStep."""
    if not isinstance(step, dict):
        raise PlanError("step must be an object", step_index=i)

    # --- Required fields ---
    for field in ("pattern", "intent", "parent_id"):
        if field not in step:
            raise PlanError("required field missing", step_index=i, field_name=field)
        value = step[field]
        if not isinstance(value, str) or not value.strip():
            raise PlanError(
                "must be a non-empty string",
                step_index=i,
                field_name=field,
            )

    # --- Validate pattern value ---
    pattern: str = step["pattern"]
    if pattern not in VALID_PATTERNS:
        valid_joined = ", ".join(sorted(VALID_PATTERNS))
        raise PlanError(
            f"unknown pattern {pattern!r}; valid: {valid_joined}",
            step_index=i,
            field_name="pattern",
        )

    # --- Optional: expression ---
    if "expression" in step:
        if not isinstance(step["expression"], str):
            raise PlanError(
                "expression must be a string",
                step_index=i,
                field_name="expression",
            )

    # --- Optional: label ---
    if "label" in step:
        value = step["label"]
        if not isinstance(value, str) or not value.strip():
            raise PlanError(
                "must be a non-empty string",
                step_index=i,
                field_name="label",
            )

    # --- Optional: contracts ---
    if "contracts" in step:
        _validate_contracts(step["contracts"], i)

    # --- Optional: metadata ---
    if "metadata" in step:
        if not isinstance(step["metadata"], dict):
            raise PlanError(
                "metadata must be a dict",
                step_index=i,
                field_name="metadata",
            )

    return step  # type: ignore[return-value]


def _validate_contracts(contracts: Any, step_index: int) -> None:
    """Validate the contracts list for a step at *step_index*."""
    if not isinstance(contracts, list):
        raise PlanError(
            "contracts must be a list",
            step_index=step_index,
            field_name="contracts",
        )

    for j, contract in enumerate(contracts):
        if not isinstance(contract, dict):
            raise PlanError(
                "contract must be an object",
                step_index=step_index,
                field_name=f"contracts[{j}]",
            )

        # kind — required, must be in _VALID_CONTRACT_KINDS
        if "kind" not in contract:
            raise PlanError(
                "required field missing",
                step_index=step_index,
                field_name=f"contracts[{j}].kind",
            )
        kind_value = contract["kind"]
        if not isinstance(kind_value, str):
            raise PlanError(
                "kind must be a string",
                step_index=step_index,
                field_name=f"contracts[{j}].kind",
            )
        if kind_value not in _VALID_CONTRACT_KINDS:
            valid_joined = ", ".join(sorted(_VALID_CONTRACT_KINDS))
            raise PlanError(
                f"unknown contract kind {kind_value!r}; valid: {valid_joined}",
                step_index=step_index,
                field_name=f"contracts[{j}].kind",
            )

        # expression — required, must be a string
        if "expression" not in contract:
            raise PlanError(
                "required field missing",
                step_index=step_index,
                field_name=f"contracts[{j}].expression",
            )
        if not isinstance(contract["expression"], str):
            raise PlanError(
                "expression must be a string",
                step_index=step_index,
                field_name=f"contracts[{j}].expression",
            )
