"""Tests for ail_agent.providers._retry.with_retry."""
from __future__ import annotations

from typing import List

import pytest

from ail_agent.errors import ProviderError
from ail_agent.providers._retry import with_retry


def test_success_on_first_attempt_no_sleep_called() -> None:
    sleeps: List[float] = []

    result = with_retry(
        lambda: "ok",
        provider="test",
        is_transient=lambda e: True,
        sleep=lambda d: sleeps.append(d),
    )

    assert result == "ok"
    assert sleeps == []


def test_transient_classifier_identifies_known_errors() -> None:
    """Verify the classifier callback is actually invoked."""
    classifier_calls: List[BaseException] = []

    def is_transient(exc: BaseException) -> bool:
        classifier_calls.append(exc)
        return True

    counter = {"n": 0}

    def func() -> str:
        counter["n"] += 1
        if counter["n"] < 2:
            raise RuntimeError("transient")
        return "done"

    result = with_retry(
        func,
        provider="test",
        is_transient=is_transient,
        delays=(0.0,),
        sleep=lambda d: None,
    )

    assert result == "done"
    assert len(classifier_calls) == 1
    assert isinstance(classifier_calls[0], RuntimeError)


def test_permanent_exception_propagates_immediately() -> None:
    calls = {"n": 0}

    def func() -> str:
        calls["n"] += 1
        raise ValueError("permanent")

    with pytest.raises(ValueError, match="permanent"):
        with_retry(
            func,
            provider="test",
            is_transient=lambda e: False,
            delays=(1.0, 2.0),
            sleep=lambda d: None,
        )

    # Only one attempt — permanent errors do not retry
    assert calls["n"] == 1


def test_two_failures_then_success_uses_correct_delays() -> None:
    sleeps: List[float] = []
    counter = {"n": 0}

    def func() -> str:
        counter["n"] += 1
        if counter["n"] <= 2:
            raise RuntimeError("transient")
        return "success"

    result = with_retry(
        func,
        provider="test",
        is_transient=lambda e: isinstance(e, RuntimeError),
        delays=(1.0, 2.0, 4.0),
        sleep=lambda d: sleeps.append(d),
    )

    assert result == "success"
    assert sleeps == [1.0, 2.0]


def test_three_failures_raises_provider_error_with_cause() -> None:
    last_exc = RuntimeError("boom")
    call_count = {"n": 0}

    def func() -> str:
        call_count["n"] += 1
        raise last_exc

    with pytest.raises(ProviderError) as exc_info:
        with_retry(
            func,
            provider="myprovider",
            is_transient=lambda e: True,
            delays=(0.0, 0.0, 0.0),
            sleep=lambda d: None,
        )

    err = exc_info.value
    assert err.__cause__ is last_exc
    assert "exhausted 4 attempts" in str(err)


def test_custom_delays_respected() -> None:
    sleeps: List[float] = []
    counter = {"n": 0}

    def func() -> str:
        counter["n"] += 1
        if counter["n"] <= 2:
            raise RuntimeError("fail")
        return "ok"

    result = with_retry(
        func,
        provider="test",
        is_transient=lambda e: True,
        delays=(0.5, 1.5),
        sleep=lambda d: sleeps.append(d),
    )

    assert result == "ok"
    assert sleeps == [0.5, 1.5]
    # 3 total attempts: initial + 2 retries
    assert counter["n"] == 3
