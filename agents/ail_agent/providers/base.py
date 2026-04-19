"""LLMProvider Protocol — the uniform interface every provider must satisfy."""
from __future__ import annotations

from typing import Any, Optional, runtime_checkable

from typing_extensions import Protocol, TypedDict


class ToolCall(TypedDict):
    """Normalized tool-call. OpenAI JSON-string ``arguments`` is parsed to a dict."""

    id: str
    name: str
    arguments: dict[str, Any]


class ToolSpec(TypedDict, total=False):
    """Caller-supplied tool schema.

    ``name`` and ``input_schema`` are required; ``description`` is optional.
    Providers translate this to their native format internally.
    """

    name: str
    description: str
    input_schema: dict[str, Any]


class CompletionResult(TypedDict):
    """Result of a tool-capable completion.

    ``text`` may be ``None`` when the model returned only tool_calls.
    """

    text: Optional[str]
    tool_calls: list[ToolCall]


@runtime_checkable
class LLMProvider(Protocol):
    """Uniform interface for LLM provider adapters.

    Implementations must be transport-agnostic. They must not import from
    ``langgraph`` or the orchestrator. API credentials must be read at call
    time — never as module-level constants.
    """

    name: str

    def complete(self, system: str, user: str, *, model: str) -> str:
        """Send a prompt and return the model's text response."""
        ...

    def complete_with_tools(
        self,
        system: str,
        user: str,
        *,
        model: str,
        tools: list[ToolSpec],
        tool_choice: Optional[str] = None,
    ) -> CompletionResult:
        """Send a prompt with tool schemas and return normalized tool calls."""
        ...
