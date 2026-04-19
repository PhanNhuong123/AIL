"""Public API for the ail_agent package."""
from __future__ import annotations

from ail_agent.errors import (
    AgentError,
    ProviderConfigError,
    ProviderError,
    RoutingError,
)
from ail_agent.orchestrator import (
    AILAgentState,
    build_workflow,
    initial_state,
    route_to_agent,
)
from ail_agent.providers.base import (
    CompletionResult,
    LLMProvider,
    ToolCall,
    ToolSpec,
)
from ail_agent.registry import get_provider, parse_model_spec

__version__ = "0.1.0"

__all__ = [
    "AILAgentState",
    "AgentError",
    "CompletionResult",
    "LLMProvider",
    "ProviderConfigError",
    "ProviderError",
    "RoutingError",
    "ToolCall",
    "ToolSpec",
    "__version__",
    "build_workflow",
    "get_provider",
    "initial_state",
    "parse_model_spec",
    "route_to_agent",
]
