"""LLM provider implementations for the AIL agent layer.

Each provider is transport-agnostic and must not import anything from
``langgraph`` or ``ail_agent.orchestrator``.  Provider selection logic lives
in ``ail_agent.registry``, not here.

Concrete provider bodies land in task 14.2.
"""
