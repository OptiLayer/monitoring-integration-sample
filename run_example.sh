#!/bin/bash
set -e

# OptiMonitor Example - Manual Workflow Script
# This script demonstrates the monitoring workflow using curl commands

MONITORING_PORT="${1:-8200}"
SPECTROMETER_PORT="${2:-8100}"

MONITORING_URL="http://localhost:${MONITORING_PORT}"
SPECTROMETER_URL="http://localhost:${SPECTROMETER_PORT}"

echo "================================================================================"
echo "OptiMonitor Example - Manual Workflow"
echo "================================================================================"
echo ""
echo "Prerequisites:"
echo "  1. Monitoring server must be running on port ${MONITORING_PORT}"
echo "  2. Virtual spectrometer must be running on port ${SPECTROMETER_PORT}"
echo ""
echo "To start them manually:"
echo "  Terminal 1: cd example && python -m monitoring.server --port ${MONITORING_PORT}"
echo "  Terminal 2: cd example && python virtual_spectrometer.py --port ${SPECTROMETER_PORT}"
echo ""
echo "================================================================================"
echo ""

read -p "Press Enter when both servers are running..."

echo ""
echo "Step 1: Checking server health..."
echo "--------------------------------------------------------------------------------"
curl -s "${MONITORING_URL}/health" | python -m json.tool
echo ""

echo ""
echo "Step 2: Connecting virtual spectrometer to monitoring API..."
echo "--------------------------------------------------------------------------------"
CONNECT_RESPONSE=$(curl -s -X POST "${MONITORING_URL}/devices/connect" \
  -H "Content-Type: application/json" \
  -d "{\"address\": \"localhost\", \"port\": ${SPECTROMETER_PORT}}")

echo "$CONNECT_RESPONSE" | python -m json.tool

DEVICE_ID=$(echo "$CONNECT_RESPONSE" | python -c "import sys, json; print(json.load(sys.stdin)['device_id'])")
SPECTROMETER_ID=$(echo "$CONNECT_RESPONSE" | python -c "import sys, json; print(json.load(sys.stdin)['spectrometer_id'])")
VACUUM_CHAMBER_ID=$(echo "$CONNECT_RESPONSE" | python -c "import sys, json; print(json.load(sys.stdin)['vacuum_chamber_id'])")

echo ""
echo "Device connected:"
echo "  Device ID: ${DEVICE_ID}"
echo "  Spectrometer ID: ${SPECTROMETER_ID}"
echo "  Vacuum Chamber ID: ${VACUUM_CHAMBER_ID}"

echo ""
echo "Step 3: Activating spectrometer..."
echo "--------------------------------------------------------------------------------"
curl -s -X POST "${MONITORING_URL}/spectrometers/${SPECTROMETER_ID}/activate" | python -m json.tool
echo ""
echo "Spectrometer activated!"

echo ""
echo "Step 4: Setting material for deposition..."
echo "--------------------------------------------------------------------------------"
curl -s -X PUT "${MONITORING_URL}/vacuum-chambers/${VACUUM_CHAMBER_ID}/material" \
  -H "Content-Type: application/json" \
  -d '{"material": "H"}' | python -m json.tool
echo ""
echo "Material set to H"

echo ""
echo "Step 5: Starting vacuum chamber (begins data generation)..."
echo "--------------------------------------------------------------------------------"
curl -s -X POST "${MONITORING_URL}/vacuum-chambers/${VACUUM_CHAMBER_ID}/start" | python -m json.tool
echo ""
echo "Vacuum chamber started - deposition in progress!"

echo ""
echo "Step 6: Fetching spectral data (5 samples)..."
echo "--------------------------------------------------------------------------------"
for i in {1..5}; do
  echo "Sample ${i}/5:"
  DATA=$(curl -s "${MONITORING_URL}/spectrometers/${SPECTROMETER_ID}/data")
  if [ "$DATA" != "null" ]; then
    NUM_POINTS=$(echo "$DATA" | python -c "import sys, json; data = json.load(sys.stdin); print(len(data['calibrated_readings']))")
    AVG_VALUE=$(echo "$DATA" | python -c "import sys, json; data = json.load(sys.stdin); readings = data['calibrated_readings']; print(f'{sum(readings)/len(readings):.2f}')")
    TIMESTAMP=$(echo "$DATA" | python -c "import sys, json; print(json.load(sys.stdin)['timestamp'])")
    echo "  ✓ Received ${NUM_POINTS} data points, avg value: ${AVG_VALUE}%, timestamp: ${TIMESTAMP}"
  else
    echo "  No data available yet"
  fi
  sleep 1
done

echo ""
echo "Step 7: Stopping vacuum chamber (ends data generation)..."
echo "--------------------------------------------------------------------------------"
curl -s -X POST "${MONITORING_URL}/vacuum-chambers/${VACUUM_CHAMBER_ID}/stop" | python -m json.tool
echo ""
echo "Vacuum chamber stopped - deposition ended!"

echo ""
echo "Step 8: Verifying data generation stopped..."
echo "--------------------------------------------------------------------------------"
sleep 2
curl -s "${SPECTROMETER_URL}/vacuum_chamber/status" | python -m json.tool

echo ""
echo "================================================================================"
echo "WORKFLOW COMPLETED SUCCESSFULLY!"
echo "================================================================================"
echo ""
echo "Summary:"
echo "  1. ✓ Connected virtual spectrometer to monitoring API"
echo "  2. ✓ Activated spectrometer"
echo "  3. ✓ Started vacuum chamber (data generation began)"
echo "  4. ✓ Received spectral data via monitoring API"
echo "  5. ✓ Stopped vacuum chamber (data generation ended)"
echo ""
echo "You can now disconnect the device:"
echo "  curl -X DELETE ${MONITORING_URL}/devices/${DEVICE_ID}"
echo ""
