"""Repository base classes for AIL-generated data access code."""

from abc import ABC, abstractmethod
from typing import Any


class AilRepository(ABC):
    """Sync repository base class for AIL-generated data access code.

    Use when EmitConfig.async_mode = false. Generated fetch/save/update/remove
    patterns call methods on this interface without a source argument — the
    emitter discards the source name; source routing is developer responsibility
    at instantiation time.
    """

    @abstractmethod
    def get(self, entity_type: type, condition: dict[str, Any]) -> Any:
        """Fetch a single entity matching condition."""
        ...

    @abstractmethod
    def save(self, entity: Any) -> None:
        """Persist a new entity."""
        ...

    @abstractmethod
    def update(
        self,
        entity_type: type,
        condition: dict[str, Any],
        fields: dict[str, Any],
    ) -> None:
        """Update fields on entities matching condition."""
        ...

    @abstractmethod
    def delete(self, entity_type: type, condition: dict[str, Any]) -> None:
        """Remove entities matching condition."""
        ...


class AsyncAilRepository(ABC):
    """Async repository base class for AIL-generated data access code.

    Use when EmitConfig.async_mode = true. Generated calls are prefixed with
    await; all methods must be async.
    """

    @abstractmethod
    async def get(self, entity_type: type, condition: dict[str, Any]) -> Any:
        """Fetch a single entity matching condition."""
        ...

    @abstractmethod
    async def save(self, entity: Any) -> None:
        """Persist a new entity."""
        ...

    @abstractmethod
    async def update(
        self,
        entity_type: type,
        condition: dict[str, Any],
        fields: dict[str, Any],
    ) -> None:
        """Update fields on entities matching condition."""
        ...

    @abstractmethod
    async def delete(self, entity_type: type, condition: dict[str, Any]) -> None:
        """Remove entities matching condition."""
        ...
