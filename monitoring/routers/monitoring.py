from __future__ import annotations

import logging

from fastapi import APIRouter, Depends

from ..deps import get_registry
from ..device_registry import DeviceRegistry
from ..models import ActiveMonitoringStatus, SpectrometerDetails, VacuumChamberDetails

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/monitoring", tags=["monitoring"])


@router.get("/active", response_model=ActiveMonitoringStatus, operation_id="getActiveMonitoring")
async def get_active_monitoring(registry: DeviceRegistry = Depends(get_registry)):
    active_spec = registry.get_active_spectrometer()
    active_chamber = registry.get_active_vacuum_chamber()

    spec_details = None
    if active_spec:
        device = registry.get_device(active_spec.device_id)
        if device:
            latest_data = registry.get_spectral_data(active_spec.id)
            spec_details = SpectrometerDetails(
                id=active_spec.id,
                device_id=active_spec.device_id,
                name=active_spec.name,
                is_monochromatic=active_spec.is_monochromatic,
                control_wavelength=active_spec.control_wavelength,
                is_active=active_spec.is_active,
                latest_data=latest_data,
                device_info=device,
            )

    chamber_details = None
    if active_chamber:
        device = registry.get_device(active_chamber.device_id)
        if device:
            chamber_details = VacuumChamberDetails(
                id=active_chamber.id,
                device_id=active_chamber.device_id,
                name=active_chamber.name,
                process_type=active_chamber.process_type,
                status=active_chamber.status,
                material=active_chamber.current_material,
                fraction=active_chamber.current_fraction,
                is_active=active_chamber.is_active,
                device_info=device,
            )

    return ActiveMonitoringStatus(
        spectrometer=spec_details,
        vacuum_chamber=chamber_details,
    )
