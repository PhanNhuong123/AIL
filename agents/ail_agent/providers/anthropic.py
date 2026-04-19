"""Anthropic provider using the official ``anthropic`` SDK."""
from __future__ import annotations

import os
from typing import Any, Optional

try:
    import anthropic
except ImportError as e:  # pragma: no cover
    raise ImportError(
        "The 'anthropic' extra is not installed. Install with: pip install ail-agent[anthropic]"
    ) from e

from ail_agent.errors import ProviderConfigError, ProviderError
from ail_agent.providers._retry import with_retry
from ail_agent.providers.base import (
    CompletionResult,
    LLMProvider,
    ToolCall,
    ToolSpec,
)

ENV_VAR: str = "ANTHROPIC_API_KEY"


def _is_transient(exc: BaseException) -> bool:
    if isinstance(
        exc,
        (
            anthropic.APIConnectionError,
            anthropic.APITimeoutError,
            anthropic.RateLimitError,
        ),
    ):
        return True
    if isinstance(exc, anthropic.APIStatusError):
        status = getattr(exc, "status_code", 0)
        return 500 <= status < 600
    return False


class AnthropicProvider:
    name: str = "anthropic"
    DEFAULT_MODEL: str = "claude-sonnet-4-20250514"

    def _client(self) -> "anthropic.Anthropic":
        api_key = os.environ.get(ENV_VAR)
        if not api_key:
            raise ProviderConfigError(
                f"missing environment variable {ENV_VAR} for provider 'anthropic'"
            )
        return anthropic.Anthropic(api_key=api_key)

    def complete(self, system: str, user: str, *, model: str) -> str:
        client = self._client()

        def call() -> str:
            resp = client.messages.create(
                model=model,
                max_tokens=4096,
                system=system,
                messages=[{"role": "user", "content": user}],
            )
            return "".join(
                block.text
                for block in resp.content
                if getattr(block, "type", None) == "text"
            )

        try:
            return with_retry(call, provider=self.name, is_transient=_is_transient)
        except ProviderError:
            raise
        except Exception as e:
            raise ProviderError(str(e), provider=self.name, cause=e) from e

    def complete_with_tools(
        self,
        system: str,
        user: str,
        *,
        model: str,
        tools: list[ToolSpec],
        tool_choice: Optional[str] = None,
    ) -> CompletionResult:
        client = self._client()
        anthropic_tools = [
            {
                "name": t["name"],
                "description": t.get("description", ""),
                "input_schema": t["input_schema"],
            }
            for t in tools
        ]
        tc: Any = {"type": "auto"}
        if tool_choice == "none":
            tc = {"type": "none"}
        elif tool_choice and tool_choice != "auto":
            tc = {"type": "tool", "name": tool_choice}

        def call() -> CompletionResult:
            resp = client.messages.create(
                model=model,
                max_tokens=4096,
                system=system,
                messages=[{"role": "user", "content": user}],
                tools=anthropic_tools,
                tool_choice=tc,
            )
            text_parts: list[str] = []
            tool_calls: list[ToolCall] = []
            for block in resp.content:
                btype = getattr(block, "type", None)
                if btype == "text":
                    text_parts.append(block.text)
                elif btype == "tool_use":
                    tool_calls.append(
                        ToolCall(
                            id=block.id,
                            name=block.name,
                            arguments=dict(block.input) if block.input else {},
                        )
                    )
            return CompletionResult(
                text="".join(text_parts) if text_parts else None,
                tool_calls=tool_calls,
            )

        try:
            return with_retry(call, provider=self.name, is_transient=_is_transient)
        except ProviderError:
            raise
        except Exception as e:
            raise ProviderError(str(e), provider=self.name, cause=e) from e


_: type[LLMProvider] = AnthropicProvider
