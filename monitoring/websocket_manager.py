"""WebSocket manager for broadcasting spectral data."""

import json
import logging
from typing import Set

from fastapi import WebSocket

logger = logging.getLogger(__name__)


class WebSocketManager:
    """Manages WebSocket connections for spectral data broadcasting."""

    def __init__(self):
        self.active_connections: Set[WebSocket] = set()

    async def connect(self, websocket: WebSocket):
        """Accept a new WebSocket connection."""
        await websocket.accept()
        self.active_connections.add(websocket)
        logger.info(f"WebSocket connected. Total connections: {len(self.active_connections)}")

    def disconnect(self, websocket: WebSocket):
        """Remove a WebSocket connection."""
        self.active_connections.discard(websocket)
        logger.info(f"WebSocket disconnected. Total connections: {len(self.active_connections)}")

    async def broadcast_spectral_data(self, spectrometer_id: str, timestamp: str, calibrated_readings: list[float]):
        """Broadcast spectral data to all connected clients."""
        if not self.active_connections:
            logger.warning("No active WebSocket connections to broadcast to")
            return

        message = {
            "spectrometer_id": spectrometer_id,
            "timestamp": timestamp,
            "calibrated_readings": calibrated_readings,
        }

        message_json = json.dumps(message)
        disconnected = set()

        logger.info(
            f"Broadcasting spectral data to {len(self.active_connections)} clients: {len(calibrated_readings)} points"
        )

        for connection in self.active_connections:
            try:
                await connection.send_text(message_json)
                logger.debug("Sent data to WebSocket client")
            except Exception as e:
                logger.warning(f"Failed to send to WebSocket client: {e}")
                disconnected.add(connection)

        for connection in disconnected:
            self.disconnect(connection)


# Global WebSocket manager instance
ws_manager = WebSocketManager()
