"""Entry point for the HORIBA iHR320 device service."""

from __future__ import annotations

import argparse
import asyncio
import logging

import uvicorn

from .driver import HoribaDriver
from .server import create_app

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


def main() -> None:
    parser = argparse.ArgumentParser(description="HORIBA iHR320 device service for OptiMonitor")
    parser.add_argument("--port", type=int, default=8100, help="Service port (default: 8100)")
    parser.add_argument("--icl-host", type=str, default="127.0.0.1", help="ICL host (default: 127.0.0.1)")
    parser.add_argument("--icl-port", type=int, default=25010, help="ICL port (default: 25010)")
    parser.add_argument("--start-icl", action="store_true", help="Auto-start ICL.exe (Windows only)")
    args = parser.parse_args()

    driver = HoribaDriver(
        icl_host=args.icl_host,
        icl_port=args.icl_port,
        start_icl=args.start_icl,
    )

    app = create_app(driver)

    logger.info(f"Starting HORIBA iHR320 service on http://localhost:{args.port}")
    logger.info(f"ICL connection: ws://{args.icl_host}:{args.icl_port}")

    uvicorn.run(app, host="0.0.0.0", port=args.port)


if __name__ == "__main__":
    main()
