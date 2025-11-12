from __future__ import annotations

from datetime import datetime
from enum import Enum
from uuid import uuid4

from pydantic import BaseModel, ConfigDict, Field


class DeviceType(str, Enum):
    SPECTROMETER = "spectrometer"
    VACUUM_CHAMBER = "vacuum-chamber"


class ProcessType(str, Enum):
    TWO_COMPONENT = "two-component"
    THREE_COMPONENT = "three-component"
    COMPOSITE_TWO_COMPONENT = "composite-two-component"


class DeviceStatus(str, Enum):
    CONNECTED = "connected"
    DISCONNECTED = "disconnected"
    ERROR = "error"


class VacuumChamberStatus(str, Enum):
    RUNNING = "running"
    STOPPED = "stopped"


class ConnectDeviceRequest(BaseModel):
    port: int = Field(..., description="Device port number")
    address: str = Field("localhost", description="Device address")

    model_config = ConfigDict(json_schema_extra={"example": {"port": 8100, "address": "localhost"}})


class DeviceInfo(BaseModel):
    id: str = Field(default_factory=lambda: str(uuid4()), description="Device unique identifier")
    type: DeviceType = Field(..., description="Device type")
    port: int = Field(..., description="Device port")
    address: str = Field(..., description="Device address")
    name: str = Field(..., description="Device name")
    status: DeviceStatus = Field(DeviceStatus.CONNECTED, description="Device connection status")
    capabilities: dict = Field(default_factory=dict, description="Device capabilities")

    model_config = ConfigDict(from_attributes=True)


class CreateSpectrometerRequest(BaseModel):
    device_id: str = Field(..., description="Reference to connected device")
    name: str = Field(..., description="Spectrometer name")

    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "device_id": "123e4567-e89b-12d3-a456-426614174000",
                "name": "Main Spectrometer",
            }
        }
    )


class SpectrometerConfig(BaseModel):
    id: str = Field(default_factory=lambda: str(uuid4()), description="Spectrometer unique identifier")
    device_id: str = Field(..., description="Reference to connected device")
    name: str = Field(..., description="Spectrometer name")
    is_monochromatic: bool = Field(False, description="Whether this is a monochromatic spectrometer")
    control_wavelength: float | None = Field(None, description="Control wavelength in nm (monochromatic only)")
    is_active: bool = Field(False, description="Whether this is the active spectrometer")
    created_at: datetime = Field(default_factory=datetime.now, description="Creation timestamp")

    model_config = ConfigDict(from_attributes=True)


class SetControlWavelengthRequest(BaseModel):
    wavelength: float = Field(..., description="Control wavelength in nm", gt=0)

    model_config = ConfigDict(json_schema_extra={"example": {"wavelength": 550.0}})


class PostSpectralDataRequest(BaseModel):
    calibrated_readings: list[float] = Field(..., description="Calibrated spectral readings (0-100%)")
    wavelengths: list[float] = Field(..., description="Wavelength grid in nm")

    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "calibrated_readings": [10.5, 25.3, 45.8, 67.2, 89.1],
                "wavelengths": [400.0, 500.0, 600.0, 700.0, 800.0],
            }
        }
    )


class SpectralData(BaseModel):
    timestamp: datetime = Field(..., description="Data timestamp")
    calibrated_readings: list[float] = Field(..., description="Calibrated spectral readings (0-100%)")
    wavelengths: list[float] = Field(..., description="Wavelength grid in nm")

    model_config = ConfigDict(from_attributes=True)


class SpectrometerDetails(BaseModel):
    id: str = Field(..., description="Spectrometer unique identifier")
    device_id: str = Field(..., description="Reference to connected device")
    name: str = Field(..., description="Spectrometer name")
    is_monochromatic: bool = Field(..., description="Whether this is a monochromatic spectrometer")
    control_wavelength: float | None = Field(None, description="Control wavelength in nm (monochromatic only)")
    is_active: bool = Field(..., description="Whether this is the active spectrometer")
    latest_data: SpectralData | None = Field(None, description="Latest spectral data")
    device_info: DeviceInfo = Field(..., description="Connected device information")

    model_config = ConfigDict(from_attributes=True)


class CreateVacuumChamberRequest(BaseModel):
    device_id: str = Field(..., description="Reference to connected device")
    name: str = Field(..., description="Vacuum chamber name")
    process_type: ProcessType = Field(..., description="Process type")

    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "device_id": "123e4567-e89b-12d3-a456-426614174001",
                "name": "Main Chamber",
                "process_type": "two-component",
            }
        }
    )


class VacuumChamberConfig(BaseModel):
    id: str = Field(default_factory=lambda: str(uuid4()), description="Vacuum chamber unique identifier")
    device_id: str = Field(..., description="Reference to connected device")
    name: str = Field(..., description="Vacuum chamber name")
    process_type: ProcessType = Field(..., description="Process type")
    current_material: str | None = Field(None, description="Currently selected material")
    current_fraction: float | None = Field(None, description="Current deposition fraction")
    status: VacuumChamberStatus = Field(VacuumChamberStatus.STOPPED, description="Chamber status")
    is_active: bool = Field(False, description="Whether this is the active vacuum chamber")
    created_at: datetime = Field(default_factory=datetime.now, description="Creation timestamp")

    model_config = ConfigDict(from_attributes=True)


class SetMaterialRequest(BaseModel):
    material: str = Field(..., description="Material abbreviation (e.g., 'H', 'L')")
    fraction: float = Field(100.0, description="Deposition fraction (0-100%)", ge=0, le=100)

    model_config = ConfigDict(json_schema_extra={"example": {"material": "H", "fraction": 100.0}})


class SetFractionRequest(BaseModel):
    fraction: float = Field(..., description="Deposition fraction", ge=0, le=1)

    model_config = ConfigDict(json_schema_extra={"example": {"fraction": 0.5}})


class VacuumChamberDetails(BaseModel):
    id: str = Field(..., description="Vacuum chamber unique identifier")
    device_id: str = Field(..., description="Reference to connected device")
    name: str = Field(..., description="Vacuum chamber name")
    process_type: ProcessType = Field(..., description="Process type")
    status: VacuumChamberStatus = Field(..., description="Chamber status")
    material: str | None = Field(None, description="Currently selected material")
    fraction: float | None = Field(None, description="Current deposition fraction")
    is_active: bool = Field(..., description="Whether this is the active vacuum chamber")
    device_info: DeviceInfo = Field(..., description="Connected device information")

    model_config = ConfigDict(from_attributes=True)


class ActiveMonitoringStatus(BaseModel):
    spectrometer: SpectrometerDetails | None = Field(None, description="Active spectrometer")
    vacuum_chamber: VacuumChamberDetails | None = Field(None, description="Active vacuum chamber")

    model_config = ConfigDict(from_attributes=True)


class DeviceConnectionResponse(BaseModel):
    device_id: str = Field(..., description="Device unique identifier")
    device_name: str = Field(..., description="Device name")
    spectrometer_id: str | None = Field(None, description="Auto-created spectrometer ID (if device has spectrometer)")
    vacuum_chamber_id: str | None = Field(
        None, description="Auto-created vacuum chamber ID (if device has vacuum chamber)"
    )

    model_config = ConfigDict(from_attributes=True)
