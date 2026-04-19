"""Agent-layer error hierarchy (AIL-G014x range).

All exceptions carry a stable ``code`` class attribute. Messages are formatted
``"[<code>] <human text>"`` so CLI output and logs always surface the code.
"""
from __future__ import annotations

from typing import Optional


class AgentError(Exception):
    """Base class for agent-layer errors. Code: AIL-G0140."""

    code: str = "AIL-G0140"

    def __init__(self, message: str, *, cause: Optional[BaseException] = None) -> None:
        formatted = f"[{self.code}] {message}"
        super().__init__(formatted)
        self.message: str = message
        if cause is not None:
            self.__cause__ = cause


class ProviderError(AgentError):
    """Raised when a provider call fails (including retry exhaustion).

    Carries the provider name and the last underlying exception via
    ``__cause__`` for diagnostics. Code: AIL-G0140.
    """

    code: str = "AIL-G0140"

    def __init__(
        self,
        message: str,
        *,
        provider: str,
        cause: Optional[BaseException] = None,
    ) -> None:
        super().__init__(f"{provider}: {message}", cause=cause)
        self.provider: str = provider


class ProviderConfigError(AgentError):
    """Raised for configuration problems: missing env var, malformed model
    spec, unknown provider prefix. Code: AIL-G0140."""

    code: str = "AIL-G0140"


class RoutingError(AgentError):
    """Raised when a routing decision cannot be made. Code: AIL-G0141.

    Not wired into the orchestrator in 14.2; reserved for task 14.3.
    """

    code: str = "AIL-G0141"
