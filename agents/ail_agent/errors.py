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


class StepBudgetError(AgentError):
    """Raised by the coder when current_step >= steps_per_plan. Code: AIL-G0143."""

    code: str = "AIL-G0143"


class PlanError(AgentError):
    """Raised when a plan response cannot be parsed or is structurally invalid.

    Carries optional ``step_index`` and ``field_name`` context to pinpoint
    exactly which step and field triggered the error. Code: AIL-G0144.
    """

    code: str = "AIL-G0144"

    def __init__(
        self,
        message: str,
        *,
        step_index: Optional[int] = None,
        field_name: Optional[str] = None,
        cause: Optional[BaseException] = None,
    ) -> None:
        if step_index is not None and field_name is not None:
            contextual = f"step {step_index} field {field_name!r}: {message}"
        elif step_index is not None:
            contextual = f"step {step_index}: {message}"
        elif field_name is not None:
            contextual = f"step None field {field_name!r}: {message}"
        else:
            contextual = message
        super().__init__(contextual, cause=cause)
        self.step_index: Optional[int] = step_index
        self.field_name: Optional[str] = field_name


class MCPConnectionError(AgentError):
    """Raised when the MCP server cannot be reached or the connection fails.

    Callers format the message with port details themselves. Code: AIL-G0145.
    """

    code: str = "AIL-G0145"

    def __init__(
        self,
        message: str,
        *,
        port: Optional[int] = None,
        cause: Optional[BaseException] = None,
    ) -> None:
        super().__init__(message, cause=cause)
        self.port: Optional[int] = port
