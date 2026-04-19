"""Provider registry — resolves ``provider:model`` spec strings to provider instances."""
from __future__ import annotations

from typing import TYPE_CHECKING, Tuple

from ail_agent.errors import ProviderConfigError

if TYPE_CHECKING:
    from ail_agent.providers.base import LLMProvider

# Canonical prefixes recognized by this registry.  ``qwen`` is an alias for
# ``alibaba`` so users can write ``qwen:qwen-max`` as a natural shorthand.
VALID_PREFIXES: tuple[str, ...] = (
    "anthropic",
    "openai",
    "ollama",
    "deepseek",
    "alibaba",
    "qwen",
)


def parse_model_spec(model_spec: str) -> Tuple[str, str]:
    """Parse ``"<prefix>:<model>"`` and return ``(prefix, model)``.

    The prefix is lowercased. Both halves must be non-empty.

    Raises :class:`ProviderConfigError` for malformed specs.
    """
    if ":" not in model_spec:
        raise ProviderConfigError(
            f"invalid model spec {model_spec!r}: expected '<provider>:<model>'"
        )
    prefix, _, model = model_spec.partition(":")
    prefix = prefix.lower()
    if not prefix:
        raise ProviderConfigError(
            f"invalid model spec {model_spec!r}: provider prefix is empty"
        )
    if not model:
        raise ProviderConfigError(
            f"invalid model spec {model_spec!r}: model name is empty"
        )
    return prefix, model


def get_provider(model_spec: str) -> Tuple["LLMProvider", str]:
    """Return ``(provider_instance, model_name)`` for the given model spec.

    ``model_spec`` format: ``"<provider>:<model-name>"``, e.g.
    ``"anthropic:claude-sonnet-4"``.

    Provider classes are imported lazily so missing optional SDK packages only
    raise an error when the specific provider is requested.

    Raises :class:`ProviderConfigError` for unknown prefixes.
    """
    prefix, model = parse_model_spec(model_spec)

    if prefix == "anthropic":
        provider = _construct_anthropic()
    elif prefix == "openai":
        provider = _construct_openai()
    elif prefix == "ollama":
        provider = _construct_ollama()
    elif prefix == "deepseek":
        provider = _construct_deepseek()
    elif prefix in ("alibaba", "qwen"):
        provider = _construct_alibaba()
    else:
        valid_list = ", ".join(sorted(VALID_PREFIXES))
        raise ProviderConfigError(
            f"unknown provider prefix {prefix!r}; valid prefixes: {valid_list}"
        )

    return provider, model


# ---------------------------------------------------------------------------
# Lazy constructors — each import is deferred to the call site so that
# missing optional SDK packages only fail when the provider is actually used.
# ---------------------------------------------------------------------------


def _construct_anthropic() -> "LLMProvider":
    from ail_agent.providers.anthropic import AnthropicProvider  # noqa: PLC0415

    return AnthropicProvider()


def _construct_openai() -> "LLMProvider":
    from ail_agent.providers.openai import OpenAIProvider  # noqa: PLC0415

    return OpenAIProvider()


def _construct_ollama() -> "LLMProvider":
    from ail_agent.providers.ollama import OllamaProvider  # noqa: PLC0415

    return OllamaProvider()


def _construct_deepseek() -> "LLMProvider":
    from ail_agent.providers.deepseek import DeepSeekProvider  # noqa: PLC0415

    return DeepSeekProvider()


def _construct_alibaba() -> "LLMProvider":
    from ail_agent.providers.alibaba import AlibabaProvider  # noqa: PLC0415

    return AlibabaProvider()
