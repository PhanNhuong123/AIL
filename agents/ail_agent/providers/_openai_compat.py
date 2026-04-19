"""Shared helpers for OpenAI-compatible providers (OpenAI, DeepSeek, Alibaba)."""
from __future__ import annotations

import json
import re
from typing import Any, Optional, TYPE_CHECKING

try:
    import openai
except ImportError as e:  # pragma: no cover
    raise ImportError(
        "The OpenAI-compatible SDK is not installed. Install one of: "
        "ail-agent[openai], ail-agent[deepseek], ail-agent[alibaba]"
    ) from e

from ail_agent.errors import ProviderError
from ail_agent.providers._retry import with_retry
from ail_agent.providers.base import CompletionResult, ToolCall, ToolSpec

if TYPE_CHECKING:
    from openai import OpenAI


def is_transient_openai(exc: BaseException) -> bool:
    """True if ``exc`` is a transient OpenAI-family error worth retrying."""
    if isinstance(
        exc,
        (
            openai.APIConnectionError,
            openai.APITimeoutError,
            openai.RateLimitError,
        ),
    ):
        return True
    if isinstance(exc, openai.APIStatusError):
        status = getattr(exc, "status_code", 0)
        return 500 <= status < 600
    return False


def openai_compatible_complete(
    client: "OpenAI",
    system: str,
    user: str,
    *,
    model: str,
    provider_name: str,
) -> str:
    def call() -> str:
        resp = client.chat.completions.create(
            model=model,
            messages=[
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        )
        return resp.choices[0].message.content or ""

    try:
        return with_retry(call, provider=provider_name, is_transient=is_transient_openai)
    except ProviderError:
        raise
    except Exception as e:
        raise ProviderError(str(e), provider=provider_name, cause=e) from e


def openai_compatible_complete_with_tools(
    client: "OpenAI",
    system: str,
    user: str,
    *,
    model: str,
    tools: list[ToolSpec],
    tool_choice: Optional[str],
    provider_name: str,
    non_tool_models: frozenset[str],
) -> CompletionResult:
    if model in non_tool_models:
        return _prompt_fallback(
            client,
            system,
            user,
            model=model,
            tools=tools,
            provider_name=provider_name,
        )

    openai_tools = [
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
    tc: Any = "auto"
    if tool_choice == "none":
        tc = "none"
    elif tool_choice and tool_choice != "auto":
        tc = {"type": "function", "function": {"name": tool_choice}}

    def call() -> CompletionResult:
        resp = client.chat.completions.create(
            model=model,
            messages=[
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            tools=openai_tools,
            tool_choice=tc,
        )
        msg = resp.choices[0].message
        tool_calls: list[ToolCall] = []
        for tc_entry in (msg.tool_calls or []):
            fn = tc_entry.function
            try:
                args: dict[str, Any] = (
                    json.loads(fn.arguments) if fn.arguments else {}
                )
            except json.JSONDecodeError as e:
                raise ProviderError(
                    f"invalid JSON in tool_call.arguments: {fn.arguments!r}",
                    provider=provider_name,
                    cause=e,
                ) from e
            tool_calls.append(
                ToolCall(id=tc_entry.id, name=fn.name, arguments=args)
            )
        return CompletionResult(text=msg.content, tool_calls=tool_calls)

    try:
        return with_retry(call, provider=provider_name, is_transient=is_transient_openai)
    except ProviderError:
        raise
    except Exception as e:
        raise ProviderError(str(e), provider=provider_name, cause=e) from e


def _prompt_fallback(
    client: "OpenAI",
    system: str,
    user: str,
    *,
    model: str,
    tools: list[ToolSpec],
    provider_name: str,
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
    raw = openai_compatible_complete(
        client, augmented_system, user, model=model, provider_name=provider_name
    )
    return _parse_fallback_response(raw, provider_name=provider_name)


def _parse_fallback_response(raw: str, *, provider_name: str) -> CompletionResult:
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
                id=f"{provider_name}-fallback-0",
                name=str(parsed["tool"]),
                arguments=dict(parsed.get("arguments", {})),
            )
        ],
    )
