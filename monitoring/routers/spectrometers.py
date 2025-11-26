from __future__ import annotations

import logging

import httpx
from fastapi import APIRouter, Depends, HTTPException

from ..deps import get_registry
from ..device_registry import DeviceRegistry
from ..models import (
    CreateSpectrometerRequest,
    DeviceType,
    PostSpectralDataRequest,
    SetControlWavelengthRequest,
    SpectralData,
    SpectrometerConfig,
    SpectrometerDetails,
)
from ..websocket_manager import ws_manager

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/spectrometers", tags=["spectrometers"])


@router.post("/", response_model=SpectrometerConfig, status_code=201, operation_id="createSpectrometer")
async def create_spectrometer(request: CreateSpectrometerRequest, registry: DeviceRegistry = Depends(get_registry)):
    device = registry.get_device(request.device_id)
    if not device:
        raise HTTPException(status_code=404, detail=f"Device {request.device_id} not found")

    if device.type != DeviceType.SPECTROMETER:
        raise HTTPException(status_code=400, detail=f"Device {request.device_id} is not a spectrometer")

    is_monochromatic = device.capabilities.get("is_monochromatic", False)

    config = SpectrometerConfig(
        device_id=request.device_id,
        name=request.name,
        is_monochromatic=is_monochromatic,
        control_wavelength=None,
        is_active=False,
    )

    return registry.add_spectrometer(config)


@router.get("/", response_model=list[SpectrometerConfig], operation_id="listSpectrometers")
async def list_spectrometers(registry: DeviceRegistry = Depends(get_registry)):
    return registry.list_spectrometers()


@router.get("/{spectrometer_id}", response_model=SpectrometerDetails, operation_id="getSpectrometer")
async def get_spectrometer(spectrometer_id: str, registry: DeviceRegistry = Depends(get_registry)):
    config = registry.get_spectrometer(spectrometer_id)
    if not config:
        raise HTTPException(status_code=404, detail=f"Spectrometer {spectrometer_id} not found")

    device = registry.get_device(config.device_id)
    if not device:
        raise HTTPException(status_code=404, detail=f"Device {config.device_id} not found")

    latest_data = registry.get_spectral_data(spectrometer_id)

    return SpectrometerDetails(
        id=config.id,
        device_id=config.device_id,
        name=config.name,
        is_monochromatic=config.is_monochromatic,
        control_wavelength=config.control_wavelength,
        is_active=config.is_active,
        latest_data=latest_data,
        device_info=device,
    )


@router.put(
    "/{spectrometer_id}/control_wavelength", response_model=SpectrometerConfig, operation_id="setControlWavelength"
)
async def set_control_wavelength(
    spectrometer_id: str, request: SetControlWavelengthRequest, registry: DeviceRegistry = Depends(get_registry)
):
    config = registry.get_spectrometer(spectrometer_id)
    if not config:
        raise HTTPException(status_code=404, detail=f"Spectrometer {spectrometer_id} not found")

    if not config.is_monochromatic:
        raise HTTPException(status_code=400, detail="Control wavelength can only be set on monochromatic spectrometers")

    device = registry.get_device(config.device_id)
    if not device:
        raise HTTPException(status_code=404, detail=f"Device {config.device_id} not found")

    url = f"http://{device.address}:{device.port}/control_wavelength"
    try:
        async with httpx.AsyncClient() as client:
            response = await client.post(url, json={"wavelength": request.wavelength}, timeout=5.0)
            if response.status_code not in (200, 201, 204):
                logger.warning(f"Failed to set control wavelength on device: {response.status_code}")
                raise HTTPException(status_code=502, detail=f"Device returned error: {response.status_code}")
    except httpx.TimeoutException:
        logger.error(f"Timeout setting control wavelength on device at {url}")
        raise HTTPException(status_code=504, detail="Timeout communicating with device")
    except httpx.RequestError as e:
        logger.error(f"Error setting control wavelength on device: {e}")
        raise HTTPException(status_code=502, detail=f"Failed to communicate with device: {e}")

    updated = registry.update_spectrometer(spectrometer_id, control_wavelength=request.wavelength)
    if not updated:
        raise HTTPException(status_code=500, detail="Failed to update control wavelength")

    return updated


@router.post("/{spectrometer_id}/data", status_code=200, operation_id="postSpectralData")
async def post_spectral_data(
    spectrometer_id: str, request: PostSpectralDataRequest, registry: DeviceRegistry = Depends(get_registry)
):
    config = registry.get_spectrometer(spectrometer_id)
    if not config:
        raise HTTPException(status_code=404, detail=f"Spectrometer {spectrometer_id} not found")

    logger.info(f"Received spectral data: {len(request.calibrated_readings)} points")
    registry.store_spectral_data(spectrometer_id, request.calibrated_readings, request.wavelengths, request.timestamp)

    # Get the stored data to broadcast with timestamp
    stored_data = registry.get_spectral_data(spectrometer_id)
    if stored_data:
        logger.info(f"Broadcasting to {len(ws_manager.active_connections)} WebSocket clients")
        await ws_manager.broadcast_spectral_data(
            spectrometer_id, stored_data.timestamp.isoformat(), stored_data.calibrated_readings
        )

    return {}


@router.get("/{spectrometer_id}/data", response_model=SpectralData | None, operation_id="getSpectralData")
async def get_spectral_data(spectrometer_id: str, registry: DeviceRegistry = Depends(get_registry)):
    config = registry.get_spectrometer(spectrometer_id)
    if not config:
        raise HTTPException(status_code=404, detail=f"Spectrometer {spectrometer_id} not found")

    return registry.get_spectral_data(spectrometer_id)


@router.post("/{spectrometer_id}/activate", response_model=SpectrometerConfig, operation_id="activateSpectrometer")
async def activate_spectrometer(spectrometer_id: str, registry: DeviceRegistry = Depends(get_registry)):
    if not registry.set_active_spectrometer(spectrometer_id):
        raise HTTPException(status_code=404, detail=f"Spectrometer {spectrometer_id} not found")

    config = registry.get_spectrometer(spectrometer_id)
    return config


@router.delete("/{spectrometer_id}", status_code=204, operation_id="deleteSpectrometer")
async def delete_spectrometer(spectrometer_id: str, registry: DeviceRegistry = Depends(get_registry)):
    if not registry.remove_spectrometer(spectrometer_id):
        raise HTTPException(status_code=404, detail=f"Spectrometer {spectrometer_id} not found")

    return None
