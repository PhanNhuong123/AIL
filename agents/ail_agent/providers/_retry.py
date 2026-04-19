"""Hand-written exponential-backoff retry for provider calls."""
from __future__ import annotations

import logging
import time
from typing import Callable, Optional, TypeVar

from ail_agent.errors import ProviderError

T = TypeVar("T")

DEFAULT_DELAYS: tuple[float, ...] = (1.0, 2.0, 4.0)


def with_retry(
    func: Callable[[], T],
    *,
    provider: str,
    is_transient: Callable[[BaseException], bool],
    delays: tuple[float, ...] = DEFAULT_DELAYS,
    sleep: Callable[[float], None] = time.sleep,
    logger: Optional[logging.Logger] = None,
) -> T:
    """Run ``func`` with exponential backoff on transient errors.

    ``len(delays)`` retries follow the initial attempt. Non-transient
    exceptions propagate immediately. After all retries fail, raises
    :class:`ProviderError` wrapping the last exception.
    """
    log = logger or logging.getLogger(f"ail_agent.providers.{provider}")
    last_exc: Optional[BaseException] = None
    total_attempts = len(delays) + 1
    # `is_transient` MUST stay narrow: it is consulted for every BaseException,
    # so loosening it risks swallowing KeyboardInterrupt / SystemExit.
    for attempt in range(total_attempts):
        try:
            return func()
        except BaseException as exc:  # noqa: BLE001
            if not is_transient(exc):
                raise
            last_exc = exc
            if attempt < len(delays):
                delay = delays[attempt]
                log.warning(
                    "transient failure (attempt %d/%d): %s; retrying in %.1fs",
                    attempt + 1,
                    total_attempts,
                    exc,
                    delay,
                )
                sleep(delay)
            else:
                break
    assert last_exc is not None
    raise ProviderError(
        f"exhausted {total_attempts} attempts",
        provider=provider,
        cause=last_exc,
    )
