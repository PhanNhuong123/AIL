"""Tests for AilRepository and AsyncAilRepository base classes."""

import asyncio
from typing import Any

import pytest

from ail_runtime import AilRepository, AsyncAilRepository


# ---------------------------------------------------------------------------
# AilRepository (sync)
# ---------------------------------------------------------------------------


def test_sync_repository_is_abstract():
    with pytest.raises(TypeError):
        AilRepository()  # type: ignore[abstract]


def test_sync_concrete_subclass_requires_all_methods():
    class PartialRepo(AilRepository):
        def get(self, entity_type: type, condition: dict[str, Any]) -> Any:
            return None

        # missing save, update, delete

    with pytest.raises(TypeError):
        PartialRepo()


def test_sync_concrete_subclass_works():
    class FakeRepo(AilRepository):
        def get(self, entity_type: type, condition: dict[str, Any]) -> Any:
            return {"id": 1}

        def save(self, entity: Any) -> None:
            pass

        def update(
            self,
            entity_type: type,
            condition: dict[str, Any],
            fields: dict[str, Any],
        ) -> None:
            pass

        def delete(self, entity_type: type, condition: dict[str, Any]) -> None:
            pass

    repo = FakeRepo()
    assert repo.get(object, {"id": 1}) == {"id": 1}
    repo.save(object())
    repo.update(object, {"id": 1}, {"name": "x"})
    repo.delete(object, {"id": 1})


# ---------------------------------------------------------------------------
# AsyncAilRepository
# ---------------------------------------------------------------------------


def test_async_repository_is_abstract():
    with pytest.raises(TypeError):
        AsyncAilRepository()  # type: ignore[abstract]


def test_async_concrete_subclass_works():
    class FakeAsyncRepo(AsyncAilRepository):
        async def get(self, entity_type: type, condition: dict[str, Any]) -> Any:
            return {"id": 2}

        async def save(self, entity: Any) -> None:
            pass

        async def update(
            self,
            entity_type: type,
            condition: dict[str, Any],
            fields: dict[str, Any],
        ) -> None:
            pass

        async def delete(self, entity_type: type, condition: dict[str, Any]) -> None:
            pass

    async def run() -> None:
        repo = FakeAsyncRepo()
        result = await repo.get(object, {"id": 2})
        assert result == {"id": 2}
        await repo.save(object())
        await repo.update(object, {"id": 2}, {"name": "y"})
        await repo.delete(object, {"id": 2})

    asyncio.run(run())
