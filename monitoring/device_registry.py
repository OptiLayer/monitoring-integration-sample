from __future__ import annotations

import logging
from datetime import datetime

import httpx

from .models import (
    DeviceInfo,
    DeviceStatus,
    DeviceType,
    ProcessType,
    SpectralData,
    SpectrometerConfig,
    VacuumChamberConfig,
    VacuumChamberStatus,
)

logger = logging.getLogger(__name__)


class DeviceRegistry:
    def __init__(self):
        self.devices: dict[str, DeviceInfo] = {}
        self.spectrometers: dict[str, SpectrometerConfig] = {}
        self.vacuum_chambers: dict[str, VacuumChamberConfig] = {}
        self.spectral_data: dict[str, SpectralData] = {}
        logger.info("Device registry initialized")

    async def discover_device(
        self, port: int, address: str, monitoring_api_url: str
    ) -> tuple[DeviceInfo | None, str | None, str | None]:
        url = f"http://{address}:{port}/device/info"
        logger.info(f"Attempting to discover device at {url}")
        try:
            async with httpx.AsyncClient() as client:
                response = await client.get(url, timeout=5.0)
                if response.status_code == 200:
                    data = response.json()
                    device_info = DeviceInfo(
                        type=DeviceType(data["type"]),
                        port=port,
                        address=address,
                        name=data.get("name", "Unknown Device"),
                        status=DeviceStatus.CONNECTED,
                        capabilities=data.get("capabilities", {}),
                    )
                    self.devices[device_info.id] = device_info
                    logger.info(
                        f"Discovered device: {device_info.name} (type={device_info.type.value}, id={device_info.id})"
                    )

                    spectrometer_id = None
                    vacuum_chamber_id = None

                    capabilities = device_info.capabilities
                    if capabilities.get("has_spectrometer"):
                        is_monochromatic = capabilities.get("is_monochromatic", False)

                        spectrometer_config = SpectrometerConfig(
                            device_id=device_info.id,
                            name=f"{device_info.name} - Spectrometer",
                            is_monochromatic=is_monochromatic,
                            control_wavelength=None,
                            is_active=False,
                        )
                        self.add_spectrometer(spectrometer_config)
                        spectrometer_id = spectrometer_config.id
                        logger.info(f"Auto-created spectrometer: {spectrometer_config.name} (id={spectrometer_id})")

                    if capabilities.get("has_vacuum_chamber"):
                        process_type = ProcessType(capabilities.get("process_type", "two-component"))

                        vacuum_chamber_config = VacuumChamberConfig(
                            device_id=device_info.id,
                            name=f"{device_info.name} - Vacuum Chamber",
                            process_type=process_type,
                            current_material="H",
                            current_fraction=None,
                            status=VacuumChamberStatus.STOPPED,
                            is_active=False,
                        )
                        self.add_vacuum_chamber(vacuum_chamber_config)
                        vacuum_chamber_id = vacuum_chamber_config.id
                        logger.info(
                            f"Auto-created vacuum chamber: {vacuum_chamber_config.name} (id={vacuum_chamber_id}, type={process_type.value})"
                        )

                    register_url = f"http://{address}:{port}/register"
                    register_payload = {
                        "monitoring_api_url": monitoring_api_url,
                        "spectrometer_id": spectrometer_id,
                        "vacuum_chamber_id": vacuum_chamber_id,
                    }
                    try:
                        register_response = await client.post(register_url, json=register_payload, timeout=5.0)
                        if register_response.status_code == 200:
                            logger.info(
                                f"Device registered successfully - spectrometer_id={spectrometer_id}, vacuum_chamber_id={vacuum_chamber_id}"
                            )
                        else:
                            logger.warning(f"Device registration returned status {register_response.status_code}")
                    except Exception as reg_error:
                        logger.warning(f"Failed to register device (non-fatal): {reg_error}")

                    return (device_info, spectrometer_id, vacuum_chamber_id)
                else:
                    logger.warning(f"Device at {url} returned status code {response.status_code}")
                return (None, None, None)
        except httpx.TimeoutException:
            logger.error(f"Timeout connecting to device at {url}")
            return (None, None, None)
        except Exception as e:
            logger.error(f"Error discovering device at {url}: {e}")
            return (None, None, None)

    def get_device(self, device_id: str) -> DeviceInfo | None:
        return self.devices.get(device_id)

    def list_devices(self) -> list[DeviceInfo]:
        return list(self.devices.values())

    def remove_device(self, device_id: str) -> bool:
        if device_id in self.devices:
            device = self.devices[device_id]
            del self.devices[device_id]
            logger.info(f"Removed device: {device.name} (id={device_id})")
            return True
        logger.warning(f"Attempted to remove non-existent device: {device_id}")
        return False

    def add_spectrometer(self, config: SpectrometerConfig) -> SpectrometerConfig:
        self.spectrometers[config.id] = config
        logger.info(f"Added spectrometer: {config.name} (id={config.id})")
        return config

    def get_spectrometer(self, spectrometer_id: str) -> SpectrometerConfig | None:
        return self.spectrometers.get(spectrometer_id)

    def list_spectrometers(self) -> list[SpectrometerConfig]:
        return list(self.spectrometers.values())

    def update_spectrometer(self, spectrometer_id: str, **kwargs) -> SpectrometerConfig | None:
        if spectrometer_id in self.spectrometers:
            config = self.spectrometers[spectrometer_id]
            for key, value in kwargs.items():
                if hasattr(config, key):
                    setattr(config, key, value)
            logger.debug(f"Updated spectrometer {spectrometer_id}: {kwargs}")
            return config
        logger.warning(f"Attempted to update non-existent spectrometer: {spectrometer_id}")
        return None

    def set_active_spectrometer(self, spectrometer_id: str) -> bool:
        if spectrometer_id in self.spectrometers:
            for spec_id, spec in self.spectrometers.items():
                spec.is_active = spec_id == spectrometer_id
            logger.info(f"Set active spectrometer: {self.spectrometers[spectrometer_id].name} (id={spectrometer_id})")
            return True
        logger.warning(f"Attempted to activate non-existent spectrometer: {spectrometer_id}")
        return False

    def get_active_spectrometer(self) -> SpectrometerConfig | None:
        for spec in self.spectrometers.values():
            if spec.is_active:
                return spec
        return None

    def remove_spectrometer(self, spectrometer_id: str) -> bool:
        if spectrometer_id in self.spectrometers:
            spec = self.spectrometers[spectrometer_id]
            del self.spectrometers[spectrometer_id]
            if spectrometer_id in self.spectral_data:
                del self.spectral_data[spectrometer_id]
            logger.info(f"Removed spectrometer: {spec.name} (id={spectrometer_id})")
            return True
        logger.warning(f"Attempted to remove non-existent spectrometer: {spectrometer_id}")
        return False

    def store_spectral_data(
        self, spectrometer_id: str, calibrated_readings: list[float], wavelengths: list[float], timestamp: datetime
    ):
        self.spectral_data[spectrometer_id] = SpectralData(
            timestamp=timestamp,
            calibrated_readings=calibrated_readings,
            wavelengths=wavelengths,
        )
        logger.debug(f"Stored spectral data for spectrometer {spectrometer_id}: {len(calibrated_readings)} points")

    def get_spectral_data(self, spectrometer_id: str) -> SpectralData | None:
        return self.spectral_data.get(spectrometer_id)

    def add_vacuum_chamber(self, config: VacuumChamberConfig) -> VacuumChamberConfig:
        self.vacuum_chambers[config.id] = config
        logger.info(f"Added vacuum chamber: {config.name} (id={config.id})")
        return config

    def get_vacuum_chamber(self, chamber_id: str) -> VacuumChamberConfig | None:
        return self.vacuum_chambers.get(chamber_id)

    def list_vacuum_chambers(self) -> list[VacuumChamberConfig]:
        return list(self.vacuum_chambers.values())

    def update_vacuum_chamber(self, chamber_id: str, **kwargs) -> VacuumChamberConfig | None:
        if chamber_id in self.vacuum_chambers:
            config = self.vacuum_chambers[chamber_id]
            for key, value in kwargs.items():
                if hasattr(config, key):
                    setattr(config, key, value)
            logger.debug(f"Updated vacuum chamber {chamber_id}: {kwargs}")
            return config
        logger.warning(f"Attempted to update non-existent vacuum chamber: {chamber_id}")
        return None

    def set_active_vacuum_chamber(self, chamber_id: str) -> bool:
        if chamber_id in self.vacuum_chambers:
            for ch_id, chamber in self.vacuum_chambers.items():
                chamber.is_active = ch_id == chamber_id
            logger.info(f"Set active vacuum chamber: {self.vacuum_chambers[chamber_id].name} (id={chamber_id})")
            return True
        logger.warning(f"Attempted to activate non-existent vacuum chamber: {chamber_id}")
        return False

    def get_active_vacuum_chamber(self) -> VacuumChamberConfig | None:
        for chamber in self.vacuum_chambers.values():
            if chamber.is_active:
                return chamber
        return None

    def remove_vacuum_chamber(self, chamber_id: str) -> bool:
        if chamber_id in self.vacuum_chambers:
            chamber = self.vacuum_chambers[chamber_id]
            del self.vacuum_chambers[chamber_id]
            logger.info(f"Removed vacuum chamber: {chamber.name} (id={chamber_id})")
            return True
        logger.warning(f"Attempted to remove non-existent vacuum chamber: {chamber_id}")
        return False
