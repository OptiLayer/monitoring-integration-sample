"""Generate OpenAPI schema for the Monitoring API."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from fastapi.openapi.utils import get_openapi


def dump(schema_path: str | Path = "docs/monitoring/openapi.json") -> None:
    """Generate and save OpenAPI schema from the monitoring server app."""
    from .server import create_app

    app = create_app()

    schema = get_openapi(
        title=app.title,
        version=app.version,
        description=app.description,
        routes=app.routes,
    )

    output_path = Path(schema_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(schema, indent=2))
    print(f"OpenAPI schema written to {output_path}")


def main() -> None:
    ap = argparse.ArgumentParser(description="Generate OpenAPI schema for Monitoring API")
    ap.add_argument("-o", "--output", default="docs/monitoring/openapi.json", help="Output file path")
    args = ap.parse_args()
    dump(args.output)


if __name__ == "__main__":
    main()
