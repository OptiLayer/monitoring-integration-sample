"""Tests for the FastAPI device service endpoints."""

from __future__ import annotations

from unittest.mock import AsyncMock, patch

import numpy as np
import pytest
from httpx import ASGITransport, AsyncClient

from horiba_service.driver import AcquisitionConfig, HoribaDriver, SpectrumData
from horiba_service.server import create_app


@pytest.fixture
def mock_driver():
    driver = HoribaDriver.__new__(HoribaDriver)
    driver._connected = True
    driver._config = AcquisitionConfig()
    driver._icl_host = "localhost"
    driver._icl_port = 25010
    driver._start_icl = False
    driver._device_manager = None
    driver._mono = None
    driver._ccd = None
    driver._chip_width = 1024
    driver._chip_height = 256
    return driver


@pytest.fixture
def app(mock_driver):
    return create_app(mock_driver)


@pytest.fixture
async def client(app):
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as c:
        yield c


@pytest.mark.asyncio
async def test_device_info(client):
    resp = await client.get("/device/info")
    assert resp.status_code == 200
    data = resp.json()
    assert data["name"] == "HORIBA iHR320"
    assert data["capabilities"]["has_spectrometer"] is True
    assert data["capabilities"]["is_monochromatic"] is False


@pytest.mark.asyncio
async def test_register(client):
    resp = await client.post("/register", json={
        "monitoring_api_url": "http://localhost:8200",
        "spectrometer_id": "spec-123",
        "vacuum_chamber_id": "vc-456",
    })
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "registered"
    assert data["spectrometer_id"] == "spec-123"


@pytest.mark.asyncio
async def test_vacuum_chamber_status(client):
    resp = await client.get("/vacuum_chamber/status")
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "stopped"
    assert data["is_depositing"] is False


@pytest.mark.asyncio
async def test_calibration_status(client):
    resp = await client.get("/calibration/status")
    data = resp.json()
    assert data["is_calibrated"] is False

@pytest.mark.asyncio
async def test_calibration_reset(client):
    resp = await client.post("/calibration/reset")
    assert resp.json()["status"] == "reset"


@pytest.mark.asyncio
async def test_get_config(client):
    resp = await client.get("/config")
    assert resp.status_code == 200
    data = resp.json()
    assert "center_wavelength" in data
    assert "exposure_time_ms" in data


@pytest.mark.asyncio
async def test_material_endpoints(client):
    resp = await client.get("/vacuum_chamber/material")
    assert resp.status_code == 200

    resp = await client.post("/vacuum_chamber/material", json={"material": "L"})
    assert resp.status_code == 200
