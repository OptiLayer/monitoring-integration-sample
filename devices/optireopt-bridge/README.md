# OptiReOpt Bridge

A virtual spectrometer device that subscribes to OptiReOpt's processed-spectrum
broadcaster and forwards every frame to OptiMonitor as if it came from a real
device.

## Endpoints

- `GET /` — live spectrum dashboard
- `GET /ws` — WebSocket stream of `{type: "scan" | "source_status" | "init", ...}`
- `GET /device/info` — capabilities (broadband spectrometer, no vacuum chamber)
- `POST /register` — accept `{monitoring_api_url, spectrometer_id, vacuum_chamber_id}`
- `GET|POST /config` — read/update `{source_url, reconnect_ms}`
- `GET /latest` — most recently received frame (or 204 if none yet)

## Usage

```bash
cd devices/optireopt-bridge
cargo run -- --port 8100 --source ws://127.0.0.1:9100
# open http://localhost:8100/
```

The OptiReOpt side must be running with the
[`spectrum_broadcaster`](../../../OptiReOpt/spectrum_broadcaster.py) module
enabled (default — set `OPTIREOPT_BROADCAST=0` to disable).
