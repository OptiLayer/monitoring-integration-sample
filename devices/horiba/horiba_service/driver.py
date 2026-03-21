"""HORIBA iHR320 + CCD driver wrapper around EzSpec SDK."""

from __future__ import annotations

import asyncio
import logging
from dataclasses import dataclass, field
from typing import Callable

import numpy as np
import numpy.typing as npt

from horiba_sdk.core.acquisition_format import AcquisitionFormat
from horiba_sdk.core.timer_resolution import TimerResolution
from horiba_sdk.core.x_axis_conversion_type import XAxisConversionType
from horiba_sdk.devices.device_manager import DeviceManager
from horiba_sdk.devices.single_devices.ccd import ChargeCoupledDevice
from horiba_sdk.devices.single_devices.monochromator import Monochromator

logger = logging.getLogger(__name__)


@dataclass
class AcquisitionConfig:
    center_wavelength: float = 550.0
    exposure_time_ms: float = 100.0
    gain_token: int = 0
    speed_token: int = 0
    grating: Monochromator.Grating = Monochromator.Grating.FIRST


@dataclass
class SpectrumData:
    wavelengths: list[float] = field(default_factory=list)
    intensities: list[float] = field(default_factory=list)


class HoribaDriver:
    """Wraps the HORIBA EzSpec SDK for broadband spectral acquisition."""

    def __init__(self, icl_host: str = "127.0.0.1", icl_port: int = 25010, start_icl: bool = False):
        self._icl_host = icl_host
        self._icl_port = icl_port
        self._start_icl = start_icl
        self._device_manager: DeviceManager | None = None
        self._mono: Monochromator | None = None
        self._ccd: ChargeCoupledDevice | None = None
        self._config = AcquisitionConfig()
        self._connected = False
        self._chip_width: int = 1024
        self._chip_height: int = 256

    @property
    def connected(self) -> bool:
        return self._connected

    @property
    def config(self) -> AcquisitionConfig:
        return self._config

    async def connect(self) -> None:
        """Connect to ICL, discover and open devices."""
        self._device_manager = DeviceManager(
            start_icl=self._start_icl,
            icl_ip=self._icl_host,
            icl_port=self._icl_port,
        )
        await self._device_manager.start()

        if not self._device_manager.monochromators:
            raise RuntimeError("No monochromator found")
        if not self._device_manager.charge_coupled_devices:
            raise RuntimeError("No CCD detector found")

        self._mono = self._device_manager.monochromators[0]
        self._ccd = self._device_manager.charge_coupled_devices[0]

        await self._mono.open()
        await self._wait_mono()

        if not await self._mono.is_initialized():
            logger.info("Initializing monochromator...")
            await self._mono.initialize()
            await self._wait_mono()

        await self._ccd.open()

        ccd_config = await self._ccd.get_configuration()
        self._chip_width = int(ccd_config["chipWidth"])
        self._chip_height = int(ccd_config["chipHeight"])
        logger.info(f"CCD chip: {self._chip_width}x{self._chip_height}")

        self._connected = True
        logger.info("HORIBA driver connected")

    async def disconnect(self) -> None:
        """Close devices and stop device manager."""
        if self._ccd:
            await self._ccd.close()
        if self._mono:
            await self._mono.close()
        if self._device_manager:
            await self._device_manager.stop()
        self._connected = False
        logger.info("HORIBA driver disconnected")

    async def configure(self, config: AcquisitionConfig | None = None) -> None:
        """Apply acquisition configuration."""
        if not self._connected:
            raise RuntimeError("Not connected")

        if config:
            self._config = config

        # Monochromator: grating + wavelength
        current_grating = await self._mono.get_turret_grating()
        if current_grating != self._config.grating:
            await self._mono.set_turret_grating(self._config.grating)
            await self._wait_mono()

        await self._mono.move_to_target_wavelength(self._config.center_wavelength)
        await self._wait_mono()

        # CCD: ROI (full width, Y-binned to 1 row), exposure, gain, speed
        await self._ccd.set_acquisition_format(1, AcquisitionFormat.SPECTRA_IMAGE)
        await self._ccd.set_region_of_interest(
            1, 0, 0, self._chip_width, self._chip_height, 1, self._chip_height
        )
        await self._ccd.set_center_wavelength(self._mono.id(), self._config.center_wavelength)
        await self._ccd.set_x_axis_conversion_type(XAxisConversionType.FROM_ICL_SETTINGS_INI)
        await self._ccd.set_acquisition_count(1)
        await self._ccd.set_timer_resolution(TimerResolution.MILLISECONDS)
        await self._ccd.set_exposure_time(self._config.exposure_time_ms)
        await self._ccd.set_gain(self._config.gain_token)
        await self._ccd.set_speed(self._config.speed_token)

        logger.info(
            f"Configured: wl={self._config.center_wavelength}nm, "
            f"exposure={self._config.exposure_time_ms}ms"
        )

    async def acquire(self, open_shutter: bool = True) -> SpectrumData:
        """Acquire a single spectrum.

        Args:
            open_shutter: If True (default), shutter opens during acquisition.
                          Set to False for dark reference capture.
        """
        if not self._connected:
            raise RuntimeError("Not connected")

        if not await self._ccd.get_acquisition_ready():
            raise RuntimeError("CCD not ready for acquisition")

        await self._ccd.acquisition_start(open_shutter=open_shutter)

        # Poll until done
        while await self._ccd.get_acquisition_busy():
            await asyncio.sleep(0.05)

        raw_data = await self._ccd.get_acquisition_data()

        roi = raw_data["acquisition"][0]["roi"][0]
        wavelengths = roi["xData"]
        # yData is a list of rows — we Y-binned to 1 row
        intensities = roi["yData"][0] if roi["yData"] else []

        # xData may be nested in a list
        if wavelengths and isinstance(wavelengths[0], list):
            wavelengths = wavelengths[0]

        return SpectrumData(wavelengths=wavelengths, intensities=intensities)

    async def acquire_dark(self, count: int = 10) -> npt.NDArray[np.float64]:
        """Acquire dark reference: shutter closed, average N scans."""
        scans = []
        for _ in range(count):
            spectrum = await self.acquire(open_shutter=False)
            scans.append(np.array(spectrum.intensities, dtype=np.float64))
        return np.mean(scans, axis=0)

    async def acquire_white(self, count: int = 10) -> npt.NDArray[np.float64]:
        """Acquire white/lamp reference: shutter open, no sample, average N scans."""
        scans = []
        for _ in range(count):
            spectrum = await self.acquire(open_shutter=True)
            scans.append(np.array(spectrum.intensities, dtype=np.float64))
        return np.mean(scans, axis=0)

    async def _wait_mono(self, timeout: float = 30.0) -> None:
        """Wait for monochromator to finish moving."""
        elapsed = 0.0
        while await self._mono.is_busy():
            await asyncio.sleep(0.2)
            elapsed += 0.2
            if elapsed > timeout:
                raise TimeoutError("Monochromator busy timeout")
