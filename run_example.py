#!/usr/bin/env python3
"""
Example script demonstrating the complete monitoring workflow:
1. Start monitoring server
2. Start virtual spectrometer with vacuum chamber
3. Connect device to monitoring
4. Activate spectrometer
5. Start vacuum chamber (begins data generation)
6. Monitor data flow
7. Stop vacuum chamber (ends data generation)
8. Clean shutdown
"""

import asyncio
import logging
import sys
import time
from threading import Thread

import httpx
import uvicorn

logging.basicConfig(level=logging.INFO, format="%(asctime)s - %(name)s - %(levelname)s - %(message)s")
logger = logging.getLogger(__name__)


def start_monitoring_server(port: int = 8200):
    """Start the monitoring API server in a background thread."""
    sys.path.insert(0, ".")
    from monitoring.server import create_app

    app = create_app()
    config = uvicorn.Config(app, host="0.0.0.0", port=port, log_level="info")
    server = uvicorn.Server(config)

    def run():
        asyncio.run(server.serve())

    thread = Thread(target=run, daemon=True)
    thread.start()
    logger.info(f"Monitoring server starting on port {port}")
    return thread


def start_virtual_spectrometer(port: int = 8100):
    """Start the virtual spectrometer in a background thread."""
    from virtual_spectrometer import VirtualSpectrometer, create_app

    spectrometer = VirtualSpectrometer(
        name="Example Virtual Spectrometer",
        num_points=2048,
        update_interval=0.5,
        has_spectrometer=True,
        has_vacuum_chamber=True,
    )

    app = create_app(spectrometer)
    config = uvicorn.Config(app, host="0.0.0.0", port=port, log_level="info")
    server = uvicorn.Server(config)

    def run():
        asyncio.run(server.serve())

    thread = Thread(target=run, daemon=True)
    thread.start()
    logger.info(f"Virtual spectrometer starting on port {port}")
    return thread


async def wait_for_server(url: str, timeout: int = 10, health_endpoint: str = "/health"):
    """Wait for a server to become available."""
    start_time = time.time()
    async with httpx.AsyncClient() as client:
        while time.time() - start_time < timeout:
            try:
                response = await client.get(f"{url}{health_endpoint}", timeout=2.0)
                if response.status_code == 200:
                    logger.info(f"Server at {url} is ready")
                    return True
            except Exception:
                await asyncio.sleep(0.5)
    logger.error(f"Server at {url} did not become ready in {timeout}s")
    return False


async def run_workflow():
    """Execute the complete monitoring workflow."""
    monitoring_port = 8200
    spectrometer_port = 8100
    monitoring_url = f"http://localhost:{monitoring_port}"
    spectrometer_url = f"http://localhost:{spectrometer_port}"

    logger.info("=" * 80)
    logger.info("STARTING MONITORING EXAMPLE WORKFLOW")
    logger.info("=" * 80)

    logger.info("\nStep 1: Starting servers...")
    monitoring_thread = start_monitoring_server(monitoring_port)
    time.sleep(1)
    spectrometer_thread = start_virtual_spectrometer(spectrometer_port)

    logger.info("\nStep 2: Waiting for servers to be ready...")
    if not await wait_for_server(monitoring_url):
        logger.error("Monitoring server failed to start")
        return
    if not await wait_for_server(spectrometer_url, health_endpoint="/device/info"):
        logger.error("Virtual spectrometer failed to start")
        return

    async with httpx.AsyncClient() as client:
        logger.info("\nStep 3: Connecting device to monitoring API...")
        try:
            response = await client.post(
                f"{monitoring_url}/devices/connect",
                json={"address": "localhost", "port": spectrometer_port},
                timeout=10.0,
            )
            response.raise_for_status()
            connection_data = response.json()
            device_id = connection_data["device_id"]
            spectrometer_id = connection_data["spectrometer_id"]
            vacuum_chamber_id = connection_data["vacuum_chamber_id"]
            logger.info(f"Device connected successfully!")
            logger.info(f"  Device ID: {device_id}")
            logger.info(f"  Spectrometer ID: {spectrometer_id}")
            logger.info(f"  Vacuum Chamber ID: {vacuum_chamber_id}")
        except Exception as e:
            logger.error(f"Failed to connect device: {e}")
            return

        logger.info("\nStep 4: Activating spectrometer...")
        try:
            response = await client.post(f"{monitoring_url}/spectrometers/{spectrometer_id}/activate", timeout=5.0)
            response.raise_for_status()
            logger.info(f"Spectrometer activated successfully!")
        except Exception as e:
            logger.error(f"Failed to activate spectrometer: {e}")
            return

        logger.info("\nStep 5: Starting vacuum chamber (begins data generation)...")
        try:
            response = await client.post(f"{monitoring_url}/vacuum-chambers/{vacuum_chamber_id}/start", timeout=5.0)
            response.raise_for_status()
            logger.info("Vacuum chamber started - deposition in progress!")
        except Exception as e:
            logger.error(f"Failed to start vacuum chamber: {e}")
            return

        logger.info("\nStep 6: Monitoring spectral data for 10 seconds...")
        logger.info("-" * 80)
        for i in range(20):
            try:
                response = await client.get(f"{monitoring_url}/spectrometers/{spectrometer_id}/data", timeout=5.0)
                if response.status_code == 200:
                    data = response.json()
                    if data:
                        num_points = len(data["calibrated_readings"])
                        avg_value = sum(data["calibrated_readings"]) / num_points
                        logger.info(
                            f"  [{i + 1}/20] Received spectral data: {num_points} points, "
                            f"avg value: {avg_value:.2f}%, timestamp: {data['timestamp']}"
                        )
                    else:
                        logger.info(f"  [{i + 1}/20] No data available yet")
                else:
                    logger.warning(f"  [{i + 1}/20] Failed to get data: {response.status_code}")
            except Exception as e:
                logger.error(f"  [{i + 1}/20] Error fetching data: {e}")

            await asyncio.sleep(0.5)

        logger.info("-" * 80)

        logger.info("\nStep 7: Stopping vacuum chamber (ends data generation)...")
        try:
            response = await client.post(f"{monitoring_url}/vacuum-chambers/{vacuum_chamber_id}/stop", timeout=5.0)
            response.raise_for_status()
            logger.info("Vacuum chamber stopped - deposition ended!")
        except Exception as e:
            logger.error(f"Failed to stop vacuum chamber: {e}")

        logger.info("\nStep 8: Verifying data generation stopped...")
        await asyncio.sleep(2)
        try:
            response = await client.get(f"{spectrometer_url}/vacuum_chamber/status", timeout=5.0)
            status = response.json()
            logger.info(f"Vacuum chamber status: {status}")
        except Exception as e:
            logger.error(f"Failed to check status: {e}")

    logger.info("\n" + "=" * 80)
    logger.info("WORKFLOW COMPLETED SUCCESSFULLY!")
    logger.info("=" * 80)
    logger.info("\nKey takeaways:")
    logger.info("1. The monitoring API acts as a central coordinator")
    logger.info("2. Virtual spectrometer provides both spectrometer + vacuum chamber capabilities")
    logger.info("3. Starting vacuum chamber triggers data generation")
    logger.info("4. Data flows from device → monitoring API → your application")
    logger.info("5. Stopping vacuum chamber halts data generation")
    logger.info("\nPress Ctrl+C to exit...")


def main():
    """Main entry point."""
    try:
        asyncio.run(run_workflow())
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        logger.info("\nShutting down...")
        sys.exit(0)


if __name__ == "__main__":
    main()
