# OptiMonitor Integration Example

This example demonstrates how to integrate your hardware with the OptiMonitor system. It shows the complete workflow from device connection to data streaming during deposition.

## Overview

The example includes:
- **Monitoring API Server**: Central coordinator for all devices
- **Virtual Spectrometer**: Example device with both spectrometer and vacuum chamber capabilities
- **Automated Workflow**: Python script showing the complete integration flow
- **Manual Workflow**: Bash script for step-by-step exploration

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Your Application                         │
│              (Controls deposition workflow)                  │
└─────────────────────┬───────────────────────────────────────┘
                      │ REST API
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                   Monitoring API Server                      │
│           (Manages devices, routes data)                     │
│                    Port: 8200                                │
└─────────────────────┬───────────────────────────────────────┘
                      │ REST API
                      ▼
┌─────────────────────────────────────────────────────────────┐
│              Hardware Device (Your Implementation)           │
│                                                              │
│  Capabilities:                                               │
│  - Spectrometer: Provides spectral data                     │
│  - Vacuum Chamber: Controls deposition start/stop           │
│                                                              │
│                    Port: 8100                                │
└─────────────────────────────────────────────────────────────┘
```

## Installation

1. Create a virtual environment (recommended):
```bash
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
```

2. Install dependencies:
```bash
cd example
pip install -r requirements.txt
```

## Quick Start - Automated Workflow

The easiest way to see the complete workflow:

```bash
cd example
python run_example.py
```

This will:
1. Start monitoring server (port 8200)
2. Start virtual spectrometer (port 8100)
3. Connect the device to monitoring
4. Activate the spectrometer
5. Start vacuum chamber → data generation begins
6. Display incoming spectral data for 10 seconds
7. Stop vacuum chamber → data generation ends

Press `Ctrl+C` to exit when done.

## Manual Workflow - Step by Step

For a deeper understanding, run the services manually:

### Terminal 1: Start Monitoring Server
```bash
cd example
python -m monitoring.server --port 8200
```

### Terminal 2: Start Virtual Spectrometer
```bash
cd example
python virtual_spectrometer.py --port 8100
```

### Terminal 3: Run the Workflow Script
```bash
cd example
./run_example.sh 8200 8100
```

Or use curl commands directly (see the bash script for examples).

## Complete API Workflow

### 1. Device Discovery & Connection

Your device must implement the `/device/info` endpoint:

```bash
GET http://localhost:8100/device/info
```

Response:
```json
{
  "type": "monitoring-device",
  "name": "Virtual Spectrometer",
  "capabilities": {
    "has_spectrometer": true,
    "has_vacuum_chamber": true,
    "spectrometer_type": "two-component",
    "is_monochromatic": false
  }
}
```

Connect to monitoring:
```bash
POST http://localhost:8200/devices/connect
Content-Type: application/json

{
  "address": "localhost",
  "port": 8100
}
```

Response:
```json
{
  "device_id": "abc-123",
  "device_name": "Virtual Spectrometer",
  "spectrometer_id": "spec-456",
  "vacuum_chamber_id": "vc-789"
}
```

### 2. Activate Spectrometer

```bash
POST http://localhost:8200/spectrometers/{spectrometer_id}/activate
```

### 3. Start Deposition

Start the vacuum chamber to begin data generation:

```bash
POST http://localhost:8200/vacuum_chambers/{vacuum_chamber_id}/start
```

This triggers the device's `/vacuum_chamber/start` endpoint, which begins spectral data generation.

### 4. Data Flow

Once started, your device should POST spectral data to the monitoring API:

```bash
POST http://localhost:8200/spectrometers/{spectrometer_id}/data
Content-Type: application/json

{
  "calibrated_readings": [10.5, 25.3, 45.8, 67.2, 89.1, ...]
}
```

Your application can retrieve the latest data:

```bash
GET http://localhost:8200/spectrometers/{spectrometer_id}/data
```

### 5. Stop Deposition

```bash
POST http://localhost:8200/vacuum_chambers/{vacuum_chamber_id}/stop
```

This triggers the device's `/vacuum_chamber/stop` endpoint, halting data generation.

## Implementing Your Own Device

To integrate your hardware, implement a FastAPI server with these endpoints:

### Required Endpoints

#### 1. Device Information
```python
@app.get("/device/info")
async def get_device_info():
    return {
        "type": "monitoring-device",
        "name": "Your Device Name",
        "capabilities": {
            "has_spectrometer": True,
            "has_vacuum_chamber": True,
            "spectrometer_type": "two-component",  # or "three-component"
            "is_monochromatic": False
        }
    }
```

#### 2. Registration
```python
@app.post("/register")
async def register(request: RegisterRequest):
    # Store monitoring_api_url, spectrometer_id, vacuum_chamber_id
    # These are provided by the monitoring API during connection
    return {
        "status": "registered",
        "spectrometer_id": request.spectrometer_id,
        "vacuum_chamber_id": request.vacuum_chamber_id,
        "monitoring_api_url": request.monitoring_api_url
    }
```

#### 3. Vacuum Chamber Control
```python
@app.post("/vacuum_chamber/start")
async def start_deposition():
    # Start your deposition process
    # Begin sending spectral data to monitoring API
    return {"status": "running"}

@app.post("/vacuum_chamber/stop")
async def stop_deposition():
    # Stop your deposition process
    # Stop sending spectral data
    return {"status": "stopped"}

@app.get("/vacuum_chamber/status")
async def get_status():
    return {
        "status": "running" or "stopped",
        "is_depositing": True or False
    }
```

#### 4. Data Generation Loop
```python
async def data_generation_loop():
    async with httpx.AsyncClient() as client:
        while is_depositing:
            # Get spectral data from your hardware
            calibrated_readings = get_spectral_data_from_hardware()

            # Send to monitoring API
            url = f"{monitoring_api_url}/spectrometers/{spectrometer_id}/data"
            await client.post(url, json={
                "calibrated_readings": calibrated_readings
            })

            await asyncio.sleep(update_interval)
```

### Optional Endpoints

For monochromatic spectrometers:
```python
@app.post("/control_wavelength")
async def set_control_wavelength(request: ControlWavelengthRequest):
    # Set wavelength on your hardware
    return {"control_wavelength": request.wavelength}

@app.get("/control_wavelength")
async def get_control_wavelength():
    return {"control_wavelength": current_wavelength}
```

## Key Concepts

### Device Capabilities
- **has_spectrometer**: Device can provide spectral measurements
- **has_vacuum_chamber**: Device can control deposition start/stop

Your device can have one or both capabilities.

### Spectrometer Types
- `two-component`: R and T measurements
- `three-component`: R, T, and A measurements
- `composite-two-component`: Multiple measurement configurations

### Data Format
Spectral data is sent as `calibrated_readings` - an array of floats representing reflectance/transmittance values (typically 0-100%).

### Workflow Trigger
The key insight: **Starting the vacuum chamber triggers data generation**. This mirrors real deposition where measurements only make sense while material is being deposited.

## File Structure

```
example/
├── README.md                    # This file
├── requirements.txt             # Python dependencies
├── monitoring/                  # Monitoring API server
│   ├── server.py               # FastAPI application
│   ├── models.py               # Pydantic models
│   ├── device_registry.py      # Device management
│   ├── deps.py                 # Dependency injection
│   ├── websocket_manager.py    # WebSocket broadcasting
│   └── routers/                # API endpoints
│       ├── devices.py          # Device connection/disconnection
│       ├── spectrometers.py    # Spectrometer management
│       ├── vacuum_chambers.py  # Vacuum chamber control
│       └── monitoring.py       # General monitoring
├── virtual_spectrometer.py     # Example device implementation
├── run_example.py              # Automated workflow demo
└── run_example.sh              # Manual workflow script
```

## API Reference

### Monitoring API Endpoints

#### Devices
- `POST /devices/connect` - Connect a device
- `GET /devices` - List all connected devices
- `GET /devices/{device_id}` - Get device details
- `DELETE /devices/{device_id}` - Disconnect device

#### Spectrometers
- `GET /spectrometers` - List all spectrometers
- `GET /spectrometers/{id}` - Get spectrometer details
- `POST /spectrometers/{id}/activate` - Set as active spectrometer
- `POST /spectrometers/{id}/data` - Post spectral data (called by device)
- `GET /spectrometers/{id}/data` - Get latest spectral data
- `PUT /spectrometers/{id}/control_wavelength` - Set control wavelength (monochromatic only)

#### Vacuum Chambers
- `GET /vacuum_chambers` - List all vacuum chambers
- `GET /vacuum_chambers/{id}` - Get vacuum chamber details
- `POST /vacuum_chambers/{id}/activate` - Set as active chamber
- `POST /vacuum_chambers/{id}/start` - Start deposition
- `POST /vacuum_chambers/{id}/stop` - Stop deposition

#### Monitoring
- `GET /monitoring/active` - Get active spectrometer and vacuum chamber status
- `GET /health` - Health check

### WebSocket Streaming

For real-time data streaming:

```javascript
const ws = new WebSocket('ws://localhost:8200/ws/spectral-data');

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    console.log('Spectrometer:', data.spectrometer_id);
    console.log('Timestamp:', data.timestamp);
    console.log('Data points:', data.calibrated_readings.length);
};
```

## Troubleshooting

### Port Already in Use
Change the ports:
```bash
python -m monitoring.server --port 8201
python virtual_spectrometer.py --port 8101
```

### Connection Refused
Ensure both servers are running and healthy:
```bash
curl http://localhost:8200/health
curl http://localhost:8100/device/info
```

### No Data Received
1. Check that vacuum chamber is started (not just spectrometer activated)
2. Verify device is posting data to correct endpoint
3. Check logs for error messages

## Next Steps

1. **Study the Code**: Read `virtual_spectrometer.py` to understand device implementation
2. **Modify the Example**: Change data generation to match your hardware output
3. **Test Integration**: Connect your actual hardware following the same pattern
4. **Add Features**: Implement additional endpoints as needed for your system

## Support

For questions or issues:
- Review the API documentation at http://localhost:8200/docs (when server is running)
- Check the logs for detailed error messages
- Examine the example code for implementation patterns

---

**Remember**: The monitoring API is the central coordinator. Your device just needs to:
1. Advertise its capabilities
2. Accept start/stop commands
3. Send data when active
