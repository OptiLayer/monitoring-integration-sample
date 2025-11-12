from __future__ import annotations

from fastapi import HTTPException

from .device_registry import DeviceRegistry

_registry: DeviceRegistry | None = None


def set_registry(registry: DeviceRegistry) -> None:
    global _registry
    _registry = registry


def get_registry() -> DeviceRegistry:
    if _registry is None:
        raise HTTPException(status_code=500, detail="Device registry not initialized")
    return _registry
