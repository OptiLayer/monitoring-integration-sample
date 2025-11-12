from __future__ import annotations

import logging

from fastapi import APIRouter, Depends, HTTPException, Request

from ..deps import get_registry
from ..device_registry import DeviceRegistry
from ..models import ConnectDeviceRequest, DeviceConnectionResponse, DeviceInfo

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/devices", tags=["devices"])


@router.post("/connect", response_model=DeviceConnectionResponse, status_code=200, operation_id="connectDevice")
async def connect_device(
    connect_request: ConnectDeviceRequest,
    http_request: Request,
    registry: DeviceRegistry = Depends(get_registry),
):
    monitoring_api_url = f"{http_request.url.scheme}://{http_request.url.netloc}"
    device_info, spectrometer_id, vacuum_chamber_id = await registry.discover_device(
        connect_request.port, connect_request.address, monitoring_api_url
    )
    if not device_info:
        raise HTTPException(
            status_code=404, detail=f"No device found at {connect_request.address}:{connect_request.port}"
        )
    return DeviceConnectionResponse(
        device_id=device_info.id,
        device_name=device_info.name,
        spectrometer_id=spectrometer_id,
        vacuum_chamber_id=vacuum_chamber_id,
    )


@router.get("/", response_model=list[DeviceInfo], operation_id="listDevices")
async def list_devices(registry: DeviceRegistry = Depends(get_registry)):
    return registry.list_devices()


@router.get("/{device_id}", response_model=DeviceInfo, operation_id="getDevice")
async def get_device(device_id: str, registry: DeviceRegistry = Depends(get_registry)):
    device = registry.get_device(device_id)
    if not device:
        raise HTTPException(status_code=404, detail=f"Device {device_id} not found")
    return device


@router.delete("/{device_id}", status_code=204, operation_id="disconnectDevice")
async def disconnect_device(device_id: str, registry: DeviceRegistry = Depends(get_registry)):
    if not registry.remove_device(device_id):
        raise HTTPException(status_code=404, detail=f"Device {device_id} not found")
    return None
