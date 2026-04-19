"""Ollama provider (experimental) using ``httpx`` for local model inference."""
from __future__ import annotations

import json
import logging
import os
import re
from typing import Any, Optional

try:
    import httpx
except ImportError as e:  # pragma: no cover
    raise ImportError(
        "The 'ollama' extra is not installed. Install with: pip install ail-agent[ollama]"
    ) from e

from ail_agent.errors import ProviderError
from ail_agent.providers._retry import with_retry
from ail_agent.providers.base import CompletionResult, LLMProvider, ToolCall, ToolSpec

ENV_VAR_BASE_URL: str = "OLLAMA_BASE_URL"
DEFAULT_BASE_URL: str = "http://localhost:11434"

# Models that support the native Ollama tools API
TOOL_CAPABLE_MODELS: frozenset[str] = frozenset(
    {"llama3.1", "llama3.2", "llama3.3", "qwen2.5", "mistral-small"}
)

_log = logging.getLogger("ail_agent.providers.ollama")


def _is_transient(exc: BaseException) -> bool:
    """True if ``exc`` is a transient Ollama/httpx error worth retrying."""
    if isinstance(exc, (httpx.TimeoutException, httpx.ConnectError)):
        return True
    if isinstance(exc, httpx.HTTPStatusError):
        return 500 <= exc.response.status_code < 600
    return False


def _base_model_name(model: str) -> str:
    """Strip ``:tag`` suffix to get the canonical model family name."""
    return model.split(":")[0]


class OllamaProvider:
    name: str = "ollama"
    DEFAULT_MODEL: str = "llama3.1"

    def _base_url(self) -> str:
        return os.environ.get(ENV_VAR_BASE_URL, DEFAULT_BASE_URL).rstrip("/")

    def complete(self, system: str, user: str, *, model: str) -> str:
        base = self._base_url()
        url = f"{base}/api/chat"
        payload = {
            "model": model,
            "stream": False,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        }

        def call() -> str:
            with httpx.Client() as client:
                resp = client.post(url, json=payload)
                resp.raise_for_status()
                data = resp.json()
                return data["message"]["content"]

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
        base_model = _base_model_name(model)
        if base_model in TOOL_CAPABLE_MODELS:
            return self._native_tools_complete(
                system, user, model=model, tools=tools
            )
        return self._prompt_fallback_complete(
            system, user, model=model, tools=tools
        )

    def _native_tools_complete(
        self,
        system: str,
        user: str,
        *,
        model: str,
        tools: list[ToolSpec],
    ) -> CompletionResult:
        base = self._base_url()
        url = f"{base}/api/chat"
        ollama_tools = [
            {
                "type": "function",
                "function": {
                    "name": t["name"],
                    "description": t.get("description", ""),
                    "parameters": t["input_schema"],
                },
            }
            for t in tools
        ]
        payload = {
            "model": model,
            "stream": False,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "tools": ollama_tools,
        }

        def call() -> CompletionResult:
            with httpx.Client() as client:
                resp = client.post(url, json=payload)
                resp.raise_for_status()
                data = resp.json()
            msg = data["message"]
            content = msg.get("content") or None
            raw_tool_calls = msg.get("tool_calls") or []
            tool_calls: list[ToolCall] = []
            for i, tc in enumerate(raw_tool_calls):
                fn = tc.get("function", tc)
                name = fn.get("name", "")
                raw_args = fn.get("arguments", {})
                # Ollama may return arguments as a dict or a JSON string
                if isinstance(raw_args, str):
                    try:
                        args: dict[str, Any] = json.loads(raw_args)
                    except json.JSONDecodeError:
                        args = {}
                else:
                    args = dict(raw_args) if raw_args else {}
                tool_calls.append(
                    ToolCall(
                        id=tc.get("id", f"ollama-native-{i}"),
                        name=name,
                        arguments=args,
                    )
                )
            return CompletionResult(text=content, tool_calls=tool_calls)

        try:
            return with_retry(call, provider=self.name, is_transient=_is_transient)
        except ProviderError:
            raise
        except Exception as e:
            raise ProviderError(str(e), provider=self.name, cause=e) from e

    def _prompt_fallback_complete(
        self,
        system: str,
        user: str,
        *,
        model: str,
        tools: list[ToolSpec],
    ) -> CompletionResult:
        tool_schema_block = json.dumps(
            [
                {
                    "name": t["name"],
                    "description": t.get("description", ""),
                    "input_schema": t["input_schema"],
                }
                for t in tools
            ],
            indent=2,
        )
        augmented_system = (
            f"{system}\n\n"
            "You may call a tool by replying ONLY with a JSON object in a ```json fence:\n"
            "```json\n"
            '{"tool": "<name>", "arguments": {...}}\n'
            "```\n"
            "Otherwise reply with plain text.\n\n"
            f"Available tools:\n{tool_schema_block}"
        )
        raw = self.complete(augmented_system, user, model=model)
        return _parse_prompt_fallback(raw)


def _parse_prompt_fallback(raw: str) -> CompletionResult:
    m = re.search(r"```json\s*(\{.*?\})\s*```", raw, re.DOTALL)
    if not m:
        return CompletionResult(text=raw, tool_calls=[])
    try:
        parsed = json.loads(m.group(1))
    except json.JSONDecodeError:
        return CompletionResult(text=raw, tool_calls=[])
    if not isinstance(parsed, dict) or "tool" not in parsed:
        return CompletionResult(text=raw, tool_calls=[])
    return CompletionResult(
        text=None,
        tool_calls=[
            ToolCall(
                id="ollama-fallback-0",
                name=str(parsed["tool"]),
                arguments=dict(parsed.get("arguments", {})),
            )
        ],
    )


_: type[LLMProvider] = OllamaProvider
