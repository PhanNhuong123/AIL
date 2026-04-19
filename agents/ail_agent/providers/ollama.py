"""Ollama provider stub — real HTTP client wiring lands in task 14.2."""

from __future__ import annotations

from ail_agent.providers.base import LLMProvider


class OllamaProvider:
    """Stub for the Ollama local-model provider.

    Real implementation (reading ``OLLAMA_BASE_URL`` from environment,
    constructing HTTP requests to the Ollama REST API) lands in task 14.2.
    """

    name: str = "ollama"

    def complete(self, system: str, user: str, *, model: str) -> str:
        """Send a prompt to the local Ollama server and return the response text."""
        raise NotImplementedError("OllamaProvider.complete: task 14.2")


_: type[LLMProvider] = OllamaProvider  # static Protocol conformance
