"""OpenAI provider stub — real HTTP client wiring lands in task 14.2."""

from __future__ import annotations

from ail_agent.providers.base import LLMProvider


class OpenAIProvider:
    """Stub for the OpenAI provider.

    Real implementation (reading ``OPENAI_API_KEY`` from environment,
    constructing the ``openai`` SDK client, handling chat completions) lands in
    task 14.2.
    """

    name: str = "openai"

    def complete(self, system: str, user: str, *, model: str) -> str:
        """Send a prompt to OpenAI and return the response text."""
        raise NotImplementedError("OpenAIProvider.complete: task 14.2")


_: type[LLMProvider] = OpenAIProvider  # static Protocol conformance
