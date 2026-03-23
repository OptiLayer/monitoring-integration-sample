# ATmega328P Monochromatic Spectrometer Service

A Rust service for the ATmega328P-based monochromatic spectrometer with AD7793 24-bit ADC. Integrates with the OptiMonitor system via REST API.

## Features

- **Dual operating modes**: Real hardware via serial port or log file playback
- **Cross-platform**: Works on both Windows and Linux
- **Outlier exclusion**: Grubbs' test (enabled by default) for statistical outlier removal
- **Calibration**: Automatic calculation of `(sample - dark) / (full - dark) * 100`
- **Validation**: Ensures measurement integrity (`full > sample > dark`)
- **REST API**: Compatible with OptiMonitor device protocol

## Building

### Prerequisites

- Rust toolchain (1.70+): https://rustup.rs/
- On Linux: `libudev-dev` package for serial port support
  ```bash
  # Debian/Ubuntu
  sudo apt install libudev-dev

  # Fedora
  sudo dnf install systemd-devel
  ```

### Build

```bash
cd spectrometer-service

# Debug build
cargo build

# Release build (recommended)
cargo build --release
```

The binary will be at `target/release/spectrometer-service` (or `spectrometer-service.exe` on Windows).

## Running

### Serial Mode (Real Hardware)

Connect to the ATmega328P device via serial port:

```bash
# Linux
./spectrometer-service serial --device /dev/ttyUSB0

# Windows
spectrometer-service.exe serial --device COM3

# With custom baud rate (default: 38400)
./spectrometer-service serial --device /dev/ttyUSB0 --baud 115200
```

### Playback Mode (Log File)

Replay measurements from a timestamped log file:

```bash
# Real-time playback
./spectrometer-service playback --file measurements.log

# 10x speed playback
./spectrometer-service playback --file measurements.log --speed 10.0

# Loop continuously
./spectrometer-service playback --file measurements.log --loop-playback

# Combined: fast looping playback
./spectrometer-service playback --file fixtures/sample_log.txt --speed 10.0 --loop-playback
```

### List Available Serial Ports

Helpful for finding the correct port name:

```bash
./spectrometer-service --list-ports
```

Output example:
```
Available serial ports:
  /dev/ttyUSB0 - USB - USB Serial Device
  /dev/ttyACM0 - USB - Arduino Uno
```

## CLI Options

```
spectrometer-service [OPTIONS] [COMMAND]

Commands:
  serial    Connect to real hardware via serial port
  playback  Playback from log file

Options:
  -l, --listen <PORT>           HTTP server port [default: 8100]
      --host <HOST>             HTTP server host [default: 0.0.0.0]
      --list-ports              List available serial ports and exit
      --outlier-method <METHOD> Outlier exclusion: none, grubbs [default: grubbs]
      --grubbs-alpha <ALPHA>    Significance level for Grubbs test [default: 0.05]
  -h, --help                    Print help
  -V, --version                 Print version

Serial options:
  -d, --device <DEVICE>         Serial port path (e.g., COM3, /dev/ttyUSB0)
  -b, --baud <BAUD>             Baud rate [default: 38400]

Playback options:
  -f, --file <FILE>             Path to log file
  -s, --speed <SPEED>           Playback speed multiplier [default: 1.0]
      --loop-playback           Loop when file ends
```

## REST API Endpoints

The service exposes these endpoints for OptiMonitor integration:

### Device Information

```
GET /device/info
```

Returns device capabilities:
```json
{
  "is_monochromatic": true,
  "supported_wavelengths": null
}
```

### Device Registration

```
POST /register
Content-Type: application/json

{
  "monitoring_api_url": "http://localhost:8000",
  "spectrometer_id": "spec-123",
  "vacuum_chamber_id": "chamber-456"
}
```

### Control Wavelength (Dummy)

```
GET /control_wavelength
POST /control_wavelength
Content-Type: application/json

{"wavelength": 550.0}
```

### Vacuum Chamber Control

```
GET /vacuum_chamber/material
POST /vacuum_chamber/material
Content-Type: text/plain

H
```

```
POST /vacuum_chamber/start    # Start deposition
POST /vacuum_chamber/stop     # Stop deposition
GET /vacuum_chamber/status    # Get current status
```

## Log File Format

For playback mode, log files use ISO8601 timestamps:

```
2025-01-15T10:30:00.000 SERIES1 = [1000000 1000100 1000050 1000075]
2025-01-15T10:30:00.040 SERIES2 = [8000000 8000200 8000100 8000150]
2025-01-15T10:30:00.080 SERIES3 = [4000000 4000100 4000050 4000075]
2025-01-15T10:30:00.100 END_CYCLE
```

- **SERIES1**: Dark measurement (light blocked)
- **SERIES2**: Full measurement (100% light)
- **SERIES3**: Sample measurement (through sample)
- **END_CYCLE**: Marks end of measurement cycle

Timestamps control playback timing - the service calculates delays based on timestamp differences divided by the speed multiplier.

Supported timestamp formats:
- `2025-01-15T10:30:00.123` (milliseconds)
- `2025-01-15T10:30:00.123456` (microseconds)
- `2025-01-15T10:30:00.123Z` (UTC timezone)
- `2025-01-15T10:30:00.123+00:00` (timezone offset)

## Data Processing

### Measurement Cycle

Each cycle consists of three series:
1. **Dark** (SERIES1): Baseline with no light
2. **Full** (SERIES2): Reference with full light
3. **Sample** (SERIES3): Actual measurement through sample

### Outlier Exclusion

Grubbs' test iteratively removes statistical outliers from each series before averaging. This improves measurement accuracy by excluding anomalous readings.

Disable with `--outlier-method none` if needed.

### Calibration Formula

```
calibrated_value = (sample_mean - dark_mean) / (full_mean - dark_mean) * 100
```

Result is a percentage: 0% = fully opaque, 100% = fully transparent.

### Validation

Measurements are validated to ensure:
- `full > sample` (sample blocks some light)
- `sample > dark` (sample transmits some light)

Invalid measurements are logged but not sent to OptiMonitor.

## Testing

Run the test suite:

```bash
cargo test
```

93 tests cover:
- Protocol parsing
- Outlier exclusion (Grubbs' test)
- Calibration calculations
- Validation logic
- API handlers
- Configuration parsing

## Platform Notes

### Windows

- Serial ports use `COM1`, `COM2`, `COM3`, etc.
- Use `--list-ports` to discover available ports
- Install USB-to-serial drivers if needed (CH340, FTDI, etc.)

### Linux

- Serial ports are typically `/dev/ttyUSB0`, `/dev/ttyACM0`, etc.
- User may need to be in `dialout` group for serial access:
  ```bash
  sudo usermod -a -G dialout $USER
  # Log out and back in for changes to take effect
  ```

## Example Usage with OptiMonitor

1. Start the spectrometer service:
   ```bash
   ./spectrometer-service --listen 8200 playback --file fixtures/sample_log.txt --speed 10.0 --loop-playback
   ```

2. In OptiMonitor, add a device pointing to `http://localhost:8200`

3. The service will:
   - Respond to device info requests
   - Accept registration from OptiMonitor
   - Process measurement cycles from the log file
   - Send calibrated readings to OptiMonitor when registered

## License

Part of the monitoring_example project.
