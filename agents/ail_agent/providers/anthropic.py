"""Anthropic provider stub — real HTTP client wiring lands in task 14.2."""

from __future__ import annotations

from ail_agent.providers.base import LLMProvider


class AnthropicProvider:
    """Stub for the Anthropic Claude provider.

    Real implementation (reading ``ANTHROPIC_API_KEY`` from environment,
    constructing the ``anthropic`` SDK client, streaming responses) lands in
    task 14.2.
    """

    name: str = "anthropic"

    def complete(self, system: str, user: str, *, model: str) -> str:
        """Send a prompt to Anthropic Claude and return the response text."""
        raise NotImplementedError("AnthropicProvider.complete: task 14.2")


_: type[LLMProvider] = AnthropicProvider  # static Protocol conformance
