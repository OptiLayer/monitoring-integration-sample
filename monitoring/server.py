from __future__ import annotations

import argparse
import logging

import uvicorn
from fastapi import FastAPI, WebSocket

from monitoring import deps

from .device_registry import DeviceRegistry
from .routers import devices, monitoring, spectrometers, vacuum_chambers
from .websocket_manager import ws_manager

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


def create_app(registry: DeviceRegistry | None = None) -> FastAPI:
    app = FastAPI(
        title="OptiMonitor Monitoring API",
        description="REST API for managing spectrometers and vacuum chambers",
        version="1.0.0",
    )

    if registry is None:
        registry = DeviceRegistry()

    deps.set_registry(registry)

    app.include_router(devices.router)
    app.include_router(spectrometers.router)
    app.include_router(vacuum_chambers.router)
    app.include_router(monitoring.router)

    @app.get("/health")
    async def health_check():
        return {"status": "ok"}

    @app.websocket("/ws/spectral-data")
    async def websocket_endpoint(websocket: WebSocket):
        """WebSocket endpoint for streaming spectral data."""
        await ws_manager.connect(websocket)
        try:
            while True:
                await websocket.receive_text()
        except Exception:
            ws_manager.disconnect(websocket)

    return app


def main():
    parser = argparse.ArgumentParser(description="OptiMonitor Monitoring Server")
    parser.add_argument("--port", type=int, default=8200, help="Port to run on (default: 8200)")
    parser.add_argument("--host", type=str, default="0.0.0.0", help="Host to bind to (default: 0.0.0.0)")

    args = parser.parse_args()

    logger.info(f"Starting OptiMonitor Monitoring Server on {args.host}:{args.port}")

    app = create_app()

    uvicorn.run(app, host=args.host, port=args.port)


if __name__ == "__main__":
    main()
