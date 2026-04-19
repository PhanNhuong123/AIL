"""Public API for the ail_agent package."""
from __future__ import annotations

from ail_agent.errors import (
    AgentError,
    MCPConnectionError,
    PlanError,
    ProviderConfigError,
    ProviderError,
    RoutingError,
    StepBudgetError,
)
from ail_agent.mcp_toolkit import MCPToolkit
from ail_agent.orchestrator import (
    AILAgentState,
    build_workflow,
    initial_state,
    route_to_agent,
)
from ail_agent.plan_format import VALID_PATTERNS, PlanStep, parse_plan
from ail_agent.progress import VERIFY_OK_LINE, Progress, emit
from ail_agent.prompts import PLANNER_SYSTEM_PROMPT, PLANNER_USER_TEMPLATE, PROMPT_VERSION
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
    "MCPConnectionError",
    "MCPToolkit",
    "PLANNER_SYSTEM_PROMPT",
    "PLANNER_USER_TEMPLATE",
    "PROMPT_VERSION",
    "PlanError",
    "PlanStep",
    "Progress",
    "ProviderConfigError",
    "ProviderError",
    "RoutingError",
    "StepBudgetError",
    "ToolCall",
    "ToolSpec",
    "VALID_PATTERNS",
    "VERIFY_OK_LINE",
    "__version__",
    "build_workflow",
    "emit",
    "get_provider",
    "initial_state",
    "parse_model_spec",
    "parse_plan",
    "route_to_agent",
]
