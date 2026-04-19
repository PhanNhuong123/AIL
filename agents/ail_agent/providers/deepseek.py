"""DeepSeek provider via OpenAI-compatible endpoint."""
from __future__ import annotations

import os
from typing import Optional

try:
    import openai
except ImportError as e:  # pragma: no cover
    raise ImportError(
        "The 'deepseek' extra is not installed. Install with: pip install ail-agent[deepseek]"
    ) from e

from ail_agent.errors import ProviderConfigError
from ail_agent.providers._openai_compat import (
    openai_compatible_complete,
    openai_compatible_complete_with_tools,
)
from ail_agent.providers.base import CompletionResult, LLMProvider, ToolSpec

ENV_VAR: str = "DEEPSEEK_API_KEY"
BASE_URL: str = "https://api.deepseek.com"
NON_TOOL_MODELS: frozenset[str] = frozenset({"deepseek-reasoner"})


class DeepSeekProvider:
    name: str = "deepseek"
    DEFAULT_MODEL: str = "deepseek-chat"

    def _client(self) -> "openai.OpenAI":
        api_key = os.environ.get(ENV_VAR)
        if not api_key:
            raise ProviderConfigError(
                f"missing environment variable {ENV_VAR} for provider 'deepseek'"
            )
        return openai.OpenAI(api_key=api_key, base_url=BASE_URL)

    def complete(self, system: str, user: str, *, model: str) -> str:
        return openai_compatible_complete(
            self._client(), system, user, model=model, provider_name=self.name
        )

    def complete_with_tools(
        self,
        system: str,
        user: str,
        *,
        model: str,
        tools: list[ToolSpec],
        tool_choice: Optional[str] = None,
    ) -> CompletionResult:
        return openai_compatible_complete_with_tools(
            self._client(),
            system,
            user,
            model=model,
            tools=tools,
            tool_choice=tool_choice,
            provider_name=self.name,
            non_tool_models=NON_TOOL_MODELS,
        )


_: type[LLMProvider] = DeepSeekProvider
