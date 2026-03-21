"""Tests for the HORIBA driver — unit tests with mocked SDK components."""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from horiba_service.driver import AcquisitionConfig, HoribaDriver, SpectrumData


@pytest.fixture
def mock_device_manager():
    """Create a mock DeviceManager with fake mono + CCD."""
    dm = AsyncMock()

    # Mock monochromator
    mono = AsyncMock()
    mono.id.return_value = 0
    mono.is_busy.return_value = False
    mono.is_initialized.return_value = True
    mono.get_turret_grating.return_value = MagicMock()  # Grating enum
    mono.get_current_wavelength.return_value = 550.0
    dm.monochromators = [mono]

    # Mock CCD
    ccd = AsyncMock()
    ccd.get_configuration.return_value = {"chipWidth": 1024, "chipHeight": 256}
    ccd.get_acquisition_ready.return_value = True
    ccd.get_acquisition_busy.return_value = False
    ccd.get_acquisition_data.return_value = {
        "acquisition": [{
            "roi": [{
                "xData": [list(range(1024))],
                "yData": [[float(600 + i % 10) for i in range(1024)]],
            }]
        }]
    }
    dm.charge_coupled_devices = [ccd]

    return dm


@pytest.mark.asyncio
async def test_acquire_returns_spectrum(mock_device_manager):
    """Test that acquire() returns wavelengths and intensities from CCD."""
    driver = HoribaDriver.__new__(HoribaDriver)
    driver._connected = True
    driver._config = AcquisitionConfig()
    driver._mono = mock_device_manager.monochromators[0]
    driver._ccd = mock_device_manager.charge_coupled_devices[0]
    driver._chip_width = 1024
    driver._chip_height = 256

    spectrum = await driver.acquire()

    assert len(spectrum.wavelengths) == 1024
    assert len(spectrum.intensities) == 1024
    driver._ccd.acquisition_start.assert_called_once_with(open_shutter=True)
    driver._ccd.get_acquisition_data.assert_called_once()


@pytest.mark.asyncio
async def test_configure_sets_params(mock_device_manager):
    """Test that configure() sends commands to mono + CCD."""
    driver = HoribaDriver.__new__(HoribaDriver)
    driver._connected = True
    driver._config = AcquisitionConfig()
    driver._mono = mock_device_manager.monochromators[0]
    driver._ccd = mock_device_manager.charge_coupled_devices[0]
    driver._chip_width = 1024
    driver._chip_height = 256

    config = AcquisitionConfig(center_wavelength=600.0, exposure_time_ms=200.0)
    await driver.configure(config)

    assert driver.config.center_wavelength == 600.0
    driver._mono.move_to_target_wavelength.assert_called_with(600.0)
    driver._ccd.set_exposure_time.assert_called_with(200.0)


@pytest.mark.asyncio
async def test_acquire_raises_when_not_connected():
    driver = HoribaDriver.__new__(HoribaDriver)
    driver._connected = False

    with pytest.raises(RuntimeError, match="Not connected"):
        await driver.acquire()


@pytest.mark.asyncio
async def test_configure_raises_when_not_connected():
    driver = HoribaDriver.__new__(HoribaDriver)
    driver._connected = False

    with pytest.raises(RuntimeError, match="Not connected"):
        await driver.configure()


@pytest.mark.asyncio
async def test_acquire_raises_when_ccd_not_ready(mock_device_manager):
    driver = HoribaDriver.__new__(HoribaDriver)
    driver._connected = True
    driver._config = AcquisitionConfig()
    driver._mono = mock_device_manager.monochromators[0]
    driver._ccd = mock_device_manager.charge_coupled_devices[0]
    driver._ccd.get_acquisition_ready.return_value = False

    with pytest.raises(RuntimeError, match="CCD not ready"):
        await driver.acquire()
