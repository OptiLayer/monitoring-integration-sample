# ATmega328P Monochromatic Spectrometer Service

Rust service for the ATmega328P-based monochromatic spectrometer with AD7793 24-bit ADC. Provides a calibration web UI and integrates with OptiMonitor via REST API.

## Quick Start

```bash
# Build
cargo build --release

# Playback mode (replay captured data)
cargo run -- playback --file ../putty.log --loop-playback --cycle-interval 200

# Serial mode (real device)
cargo run -- serial --device /dev/ttyUSB0

# Open calibration UI
# http://localhost:8100
```

## Calibration Workflow

1. **Start the service** in serial mode (or playback for testing)
2. **Open `http://localhost:8100`** — the calibration web UI
3. **Adjust GAIN/FADC/COUNT** in the sidebar until:
   - No **CLIPPED** badge (no saturated ADC values at 16,777,215)
   - Stable T% readings with low noise
4. **Click "Save Settings"** — writes to `calibration.toml`
5. **Next startup** automatically uses saved settings (CLI args override if provided)

The service runs calibration and monitoring simultaneously — once settings are good, connect OptiMonitor to the same service.

## Device Settings

All values from the AD7793 datasheet:

| Setting | Values | Description |
|---------|--------|-------------|
| GAIN | 1, 2, 4, 8, 16, 32, 64, 128 | ADC amplification. Higher = more sensitive but clips easier |
| FADC | 500, 250, 125, 62.5, 50, 39.2, 33.3, 19.6, 16.7, 12.5, 10, 8.33, 6.25, 4.17 Hz | Sample rate. Lower = more accurate but slower |
| COUNT | 1–12 | Measurements per series. More = better averaging, must fit in ~40ms window |

Recommended starting point: **GAIN=2, FADC=250, COUNT=4** (~38ms, 0.003% error per spec).

In serial mode, settings are sent to the device immediately when changed in the UI.

## Operating Modes

### Serial (Real Hardware)

```bash
cargo run -- serial --device /dev/ttyUSB0 [--baud 38400] [--gain 4] [--fadc 500] [--count 3]
```

- Connects to ATmega328P over serial at 38400 baud
- `--gain`, `--fadc`, `--count` override saved config if provided
- Without those flags, uses values from `calibration.toml`
- Settings changes from the web UI are sent to the device in real-time

### Playback (Log File)

```bash
cargo run -- playback --file <path> [--speed 2.0] [--loop-playback] [--cycle-interval 100]
```

Supports two log formats (auto-detected):

**Timestamped** (from the service's own logging):
```
2025-01-15T10:30:00.000 SERIES1 = [1000000 1000100 1000050]
2025-01-15T10:30:00.040 SERIES2 = [8000000 8000200 8000100]
2025-01-15T10:30:00.080 SERIES3 = [4000000 4000100 4000050]
2025-01-15T10:30:00.100 END_CYCLE
```

**Raw serial capture** (e.g., PuTTY log):
```
SERIES1 = 16777215 16777215 16777215
SERIES2 = 0 213 7
SERIES3 = 16777215 16777215 16777215
GAIN=4
FADC=500.00
COUNT=3
END_CYCLE
```

Raw logs use `--cycle-interval` (default 100ms) for pacing since there are no timestamps.

## Calibration Formula

```
T% = (sample - dark) / (full - dark) × 100
```

- **SERIES1** = dark (light blocked)
- **SERIES2** = full (100% light reference)
- **SERIES3** = sample (through material)

The AD7793 reads higher ADC values for less light (dark ~14M, full ~300). The formula handles this correctly — both numerator and denominator are negative, so they cancel out.

## Web UI

Available at `http://localhost:<port>` (default 8100).

- **Transmittance chart** — live T% over time (last 300 cycles)
- **Raw means chart** — dark (red), full (green), sample (blue) with clipping markers
- **Settings controls** — GAIN, FADC, COUNT dropdowns with Save button
- **Live values** — current T%, dark/full/sample means
- **Clipping detection** — red CLIPPED badge when any ADC value hits 16,777,215

## API Endpoints

### Calibration/Settings

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Calibration web UI |
| GET | `/ws` | WebSocket for live data streaming |
| GET | `/api/settings` | Current device settings |
| POST | `/api/settings` | Update settings (sends to device + saves to TOML) |

### OptiMonitor Integration

| Method | Path | Description |
|--------|------|-------------|
| GET | `/device/info` | Device capabilities |
| POST | `/register` | Register with monitoring API |
| GET/POST | `/control_wavelength` | Wavelength control |
| GET/POST | `/vacuum_chamber/material` | Material setting |
| POST | `/vacuum_chamber/start` | Start deposition |
| POST | `/vacuum_chamber/stop` | Stop deposition |
| GET | `/vacuum_chamber/status` | Chamber status |

## Config Persistence

Settings are saved to `calibration.toml` (configurable via `--calibration-config`):

```toml
[device_settings]
gain = 2
fadc = 250.0
count = 4

last_updated = "2026-03-23T12:00:00Z"
```

Priority: CLI args > calibration.toml > hardcoded defaults.

## Building & Testing

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # 98 tests
cargo clippy --tests     # Zero warnings
```

### Prerequisites

- Rust 2024 edition
- Linux: `libudev-dev` (`apt install libudev-dev` or `dnf install systemd-devel`)

### Serial Port Access (Linux)

```bash
sudo usermod -a -G dialout $USER
# Re-login for group change to take effect
```
