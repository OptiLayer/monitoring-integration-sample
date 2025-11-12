from __future__ import annotations

import logging

import httpx
from fastapi import APIRouter, Depends, HTTPException

from ..deps import get_registry
from ..device_registry import DeviceRegistry
from ..models import (
    CreateVacuumChamberRequest,
    DeviceType,
    SetFractionRequest,
    SetMaterialRequest,
    VacuumChamberConfig,
    VacuumChamberDetails,
    VacuumChamberStatus,
)

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/vacuum-chambers", tags=["vacuum-chambers"])


@router.post("/", response_model=VacuumChamberConfig, status_code=201, operation_id="createVacuumChamber")
async def create_vacuum_chamber(request: CreateVacuumChamberRequest, registry: DeviceRegistry = Depends(get_registry)):
    device = registry.get_device(request.device_id)
    if not device:
        raise HTTPException(status_code=404, detail=f"Device {request.device_id} not found")

    if device.type != DeviceType.VACUUM_CHAMBER:
        raise HTTPException(status_code=400, detail=f"Device {request.device_id} is not a vacuum chamber")

    config = VacuumChamberConfig(
        device_id=request.device_id,
        name=request.name,
        process_type=request.process_type,
        current_material=None,
        current_fraction=None,
        status=VacuumChamberStatus.STOPPED,
        is_active=False,
    )

    return registry.add_vacuum_chamber(config)


@router.get("/", response_model=list[VacuumChamberConfig], operation_id="listVacuumChambers")
async def list_vacuum_chambers(registry: DeviceRegistry = Depends(get_registry)):
    return registry.list_vacuum_chambers()


@router.get("/{chamber_id}", response_model=VacuumChamberDetails, operation_id="getVacuumChamber")
async def get_vacuum_chamber(chamber_id: str, registry: DeviceRegistry = Depends(get_registry)):
    config = registry.get_vacuum_chamber(chamber_id)
    if not config:
        raise HTTPException(status_code=404, detail=f"Vacuum chamber {chamber_id} not found")

    device = registry.get_device(config.device_id)
    if not device:
        raise HTTPException(status_code=404, detail=f"Device {config.device_id} not found")

    return VacuumChamberDetails(
        id=config.id,
        device_id=config.device_id,
        name=config.name,
        process_type=config.process_type,
        status=config.status,
        material=config.current_material,
        fraction=config.current_fraction,
        is_active=config.is_active,
        device_info=device,
    )


@router.post("/{chamber_id}/start", response_model=VacuumChamberConfig, operation_id="startDeposition")
async def start_deposition(chamber_id: str, registry: DeviceRegistry = Depends(get_registry)):
    config = registry.get_vacuum_chamber(chamber_id)
    if not config:
        raise HTTPException(status_code=404, detail=f"Vacuum chamber {chamber_id} not found")

    # Get the device
    device = registry.get_device(config.device_id)
    if not device:
        raise HTTPException(status_code=404, detail=f"Device {config.device_id} not found")

    try:
        async with httpx.AsyncClient() as client:
            device_url = f"http://{device.address}:{device.port}/vacuum_chamber/start"
            response = await client.post(device_url, timeout=5.0)
            response.raise_for_status()
    except Exception as e:
        logger.error(f"Failed to start deposition on device: {e}")
        raise HTTPException(status_code=500, detail=f"Failed to start deposition on device: {str(e)}")

    updated = registry.update_vacuum_chamber(chamber_id, status=VacuumChamberStatus.RUNNING)
    if not updated:
        raise HTTPException(status_code=500, detail="Failed to update chamber status")

    logger.info(f"Started deposition on vacuum chamber {chamber_id}")
    return updated


@router.post("/{chamber_id}/stop", response_model=VacuumChamberConfig, operation_id="stopDeposition")
async def stop_deposition(chamber_id: str, registry: DeviceRegistry = Depends(get_registry)):
    config = registry.get_vacuum_chamber(chamber_id)
    if not config:
        raise HTTPException(status_code=404, detail=f"Vacuum chamber {chamber_id} not found")

    # Get the device
    device = registry.get_device(config.device_id)
    if not device:
        raise HTTPException(status_code=404, detail=f"Device {config.device_id} not found")

    try:
        async with httpx.AsyncClient() as client:
            device_url = f"http://{device.address}:{device.port}/vacuum_chamber/stop"
            response = await client.post(device_url, timeout=5.0)
            response.raise_for_status()
    except Exception as e:
        logger.error(f"Failed to stop deposition on device: {e}")
        raise HTTPException(status_code=500, detail=f"Failed to stop deposition on device: {str(e)}")

    updated = registry.update_vacuum_chamber(chamber_id, status=VacuumChamberStatus.STOPPED)
    if not updated:
        raise HTTPException(status_code=500, detail="Failed to update chamber status")

    logger.info(f"Stopped deposition on vacuum chamber {chamber_id}")
    return updated


@router.put("/{chamber_id}/material", response_model=VacuumChamberConfig, operation_id="setMaterial")
async def set_material(chamber_id: str, request: SetMaterialRequest, registry: DeviceRegistry = Depends(get_registry)):
    config = registry.get_vacuum_chamber(chamber_id)
    if not config:
        raise HTTPException(status_code=404, detail=f"Vacuum chamber {chamber_id} not found")

    # Get the device
    device = registry.get_device(config.device_id)
    if not device:
        raise HTTPException(status_code=404, detail=f"Device {config.device_id} not found")

    # Forward material and fraction to device
    try:
        async with httpx.AsyncClient() as client:
            device_url = f"http://{device.address}:{device.port}/vacuum_chamber/material"
            payload = {"material": request.material, "fraction": request.fraction}
            response = await client.post(device_url, json=payload, timeout=5.0)
            response.raise_for_status()
    except Exception as e:
        logger.error(f"Failed to set material on device: {e}")
        raise HTTPException(status_code=500, detail=f"Failed to set material on device: {str(e)}")

    # Update registry cache
    updated = registry.update_vacuum_chamber(
        chamber_id, current_material=request.material, current_fraction=request.fraction
    )
    if not updated:
        raise HTTPException(status_code=500, detail="Failed to update chamber")

    logger.info(f"Set material on vacuum chamber {chamber_id}: {request.material} (fraction: {request.fraction}%)")
    return updated


@router.put("/{chamber_id}/fraction", response_model=VacuumChamberConfig, operation_id="setFraction")
async def set_fraction(chamber_id: str, request: SetFractionRequest, registry: DeviceRegistry = Depends(get_registry)):
    config = registry.get_vacuum_chamber(chamber_id)
    if not config:
        raise HTTPException(status_code=404, detail=f"Vacuum chamber {chamber_id} not found")

    updated = registry.update_vacuum_chamber(chamber_id, current_fraction=request.fraction)
    if not updated:
        raise HTTPException(status_code=500, detail="Failed to set fraction")

    logger.info(f"Set fraction on vacuum chamber {chamber_id}: {request.fraction}")
    return updated


@router.post("/{chamber_id}/activate", response_model=VacuumChamberConfig, operation_id="activateVacuumChamber")
async def activate_vacuum_chamber(chamber_id: str, registry: DeviceRegistry = Depends(get_registry)):
    if not registry.set_active_vacuum_chamber(chamber_id):
        raise HTTPException(status_code=404, detail=f"Vacuum chamber {chamber_id} not found")

    config = registry.get_vacuum_chamber(chamber_id)
    return config


@router.delete("/{chamber_id}", status_code=204, operation_id="deleteVacuumChamber")
async def delete_vacuum_chamber(chamber_id: str, registry: DeviceRegistry = Depends(get_registry)):
    if not registry.remove_vacuum_chamber(chamber_id):
        raise HTTPException(status_code=404, detail=f"Vacuum chamber {chamber_id} not found")

    return None
