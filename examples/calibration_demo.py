"""
Calibration demo — web UI replaying real TECSpec USB CSG NMOS spectrometer data.

Simulates a device serving a web UI where you can:
- Watch incoming spectra in real-time (replayed from CSV)
- See scan classification (DARK / WHITE / SUBSTRATE / TRANSITION)
- Capture dark and white references interactively
- View calibrated transmittance once references are set

Usage:
    uv run python examples/calibration_demo.py
    uv run python examples/calibration_demo.py --port 8050
    uv run python examples/calibration_demo.py --speed 10  # 10x replay speed

Then open http://localhost:8050 in your browser.
"""

from __future__ import annotations

import argparse
import asyncio
import csv
import json
import logging
import sys
from dataclasses import dataclass, field
from enum import IntEnum
from pathlib import Path

import numpy as np
import numpy.typing as npt
import uvicorn
from contextlib import asynccontextmanager
from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.responses import HTMLResponse

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Scan classification
# ---------------------------------------------------------------------------

class ScanType(IntEnum):
    DARK = 0
    WHITE = 1
    SUBSTRATE = 2
    TRANSITION = 3


# Thresholds — device-specific, adjust for your spectrometer
DARK_THRESHOLD = 200.0
WHITE_THRESHOLD = 36_000.0
SUBSTRATE_MIN = 33_000.0


def classify_scan(values: npt.NDArray[np.float64]) -> ScanType:
    mean = float(np.mean(values))
    if abs(mean) < DARK_THRESHOLD:
        return ScanType.DARK
    if mean > WHITE_THRESHOLD:
        return ScanType.WHITE
    if mean >= SUBSTRATE_MIN:
        return ScanType.SUBSTRATE
    return ScanType.TRANSITION


# ---------------------------------------------------------------------------
# Calibration state
# ---------------------------------------------------------------------------

@dataclass
class CalibrationState:
    dark_ref: npt.NDArray[np.float64] | None = None
    white_ref: npt.NDArray[np.float64] | None = None
    dark_accumulator: list[npt.NDArray[np.float64]] = field(default_factory=list)
    white_accumulator: list[npt.NDArray[np.float64]] = field(default_factory=list)
    capturing_dark: bool = False
    capturing_white: bool = False

    @property
    def is_calibrated(self) -> bool:
        return self.dark_ref is not None and self.white_ref is not None

    def calibrate(self, scan: npt.NDArray[np.float64]) -> npt.NDArray[np.float64]:
        if not self.is_calibrated:
            return np.zeros_like(scan)
        denominator = self.white_ref - self.dark_ref
        denominator = np.where(np.abs(denominator) < 1.0, 1.0, denominator)
        result = (scan - self.dark_ref) / denominator * 100.0
        return np.clip(result, 0.0, 100.0)

    def process_scan_for_capture(self, scan: npt.NDArray[np.float64], scan_type: ScanType) -> None:
        if self.capturing_dark and scan_type == ScanType.DARK:
            self.dark_accumulator.append(scan)
        if self.capturing_white and scan_type == ScanType.WHITE:
            self.white_accumulator.append(scan)

    def finalize_dark(self) -> int:
        self.capturing_dark = False
        if self.dark_accumulator:
            self.dark_ref = np.mean(self.dark_accumulator, axis=0)
        count = len(self.dark_accumulator)
        self.dark_accumulator = []
        return count

    def finalize_white(self) -> int:
        self.capturing_white = False
        if self.white_accumulator:
            self.white_ref = np.mean(self.white_accumulator, axis=0)
        count = len(self.white_accumulator)
        self.white_accumulator = []
        return count

    def reset(self) -> None:
        self.dark_ref = None
        self.white_ref = None
        self.dark_accumulator = []
        self.white_accumulator = []
        self.capturing_dark = False
        self.capturing_white = False

    def status_dict(self) -> dict:
        return {
            "is_calibrated": self.is_calibrated,
            "has_dark": self.dark_ref is not None,
            "has_white": self.white_ref is not None,
            "capturing_dark": self.capturing_dark,
            "capturing_white": self.capturing_white,
            "dark_scans_collected": len(self.dark_accumulator),
            "white_scans_collected": len(self.white_accumulator),
        }


# ---------------------------------------------------------------------------
# CSV loader
# ---------------------------------------------------------------------------

def load_csv(path: Path) -> tuple[npt.NDArray[np.float64], list[tuple[float, float, npt.NDArray[np.float64]]]]:
    """Returns (wavelengths, [(timestamp_ms, integration_time_ms, values), ...])"""
    with open(path) as f:
        reader = csv.reader(f)
        header = next(reader)
        wavelengths = np.array([float(h) for h in header[2:]], dtype=np.float64)
        scans = []
        for row in reader:
            integration_time = float(row[0])
            timestamp = float(row[1])
            values = np.array([float(v) for v in row[2:]], dtype=np.float64)
            scans.append((timestamp, integration_time, values))
    return wavelengths, scans


# ---------------------------------------------------------------------------
# Web app
# ---------------------------------------------------------------------------

def create_app(csv_path: Path, replay_speed: float = 1.0) -> FastAPI:
    wavelengths, raw_scans = load_csv(csv_path)
    logger.info(f"Loaded {len(raw_scans)} scans, {len(wavelengths)} wavelength points")
    logger.info(f"Wavelength range: {wavelengths[0]:.2f} – {wavelengths[-1]:.2f} nm")

    cal = CalibrationState()
    connected_clients: list[WebSocket] = []

    # Downsample wavelengths for browser rendering (every 4th point)
    DOWNSAMPLE = 4
    ds_wavelengths = wavelengths[::DOWNSAMPLE].tolist()

    async def broadcast(message: dict) -> None:
        dead = []
        for ws in connected_clients:
            try:
                await ws.send_json(message)
            except Exception:
                dead.append(ws)
        for ws in dead:
            connected_clients.remove(ws)

    async def replay_loop():
        """Replay CSV scans with realistic timing."""
        await asyncio.sleep(1.0)  # wait for clients to connect

        while True:
            prev_ts = raw_scans[0][0]
            for timestamp_ms, _integration_time, values in raw_scans:
                # Compute delay from timestamps
                dt_ms = timestamp_ms - prev_ts
                prev_ts = timestamp_ms
                if dt_ms > 0:
                    delay = (dt_ms / 1000.0) / replay_speed
                    # Cap delay to avoid long waits at gaps
                    delay = min(delay, 2.0 / replay_speed)
                    await asyncio.sleep(delay)

                scan_type = classify_scan(values)
                cal.process_scan_for_capture(values, scan_type)

                # Build message
                ds_values = values[::DOWNSAMPLE].tolist()
                msg: dict = {
                    "type": "scan",
                    "scan_type": scan_type.name,
                    "mean": float(np.mean(values)),
                    "values": ds_values,
                    "timestamp_ms": timestamp_ms,
                    "calibration": cal.status_dict(),
                }

                # Include calibrated data for every scan once references are set
                if cal.is_calibrated:
                    calibrated = cal.calibrate(values)
                    msg["calibrated"] = calibrated[::DOWNSAMPLE].tolist()
                    msg["calibrated_mean"] = float(np.mean(calibrated))

                await broadcast(msg)

            logger.info("Replay complete, restarting...")
            await asyncio.sleep(2.0)

    @asynccontextmanager
    async def lifespan(_app: FastAPI):
        task = asyncio.create_task(replay_loop())
        yield
        task.cancel()

    app = FastAPI(title="Calibration Demo", lifespan=lifespan)

    @app.get("/", response_class=HTMLResponse)
    async def index():
        return HTML_PAGE

    @app.get("/api/calibration/status")
    async def get_calibration_status():
        return cal.status_dict()

    @app.post("/api/calibration/dark/start")
    async def start_dark_capture():
        cal.dark_accumulator = []
        cal.capturing_dark = True
        return {"status": "capturing_dark"}

    @app.post("/api/calibration/dark/stop")
    async def stop_dark_capture():
        count = cal.finalize_dark()
        if cal.dark_ref is not None:
            await broadcast({
                "type": "dark_ref",
                "values": cal.dark_ref[::DOWNSAMPLE].tolist(),
            })
        return {"status": "done", "scans_averaged": count}

    @app.post("/api/calibration/white/start")
    async def start_white_capture():
        cal.white_accumulator = []
        cal.capturing_white = True
        return {"status": "capturing_white"}

    @app.post("/api/calibration/white/stop")
    async def stop_white_capture():
        count = cal.finalize_white()
        if cal.white_ref is not None:
            await broadcast({
                "type": "white_ref",
                "values": cal.white_ref[::DOWNSAMPLE].tolist(),
            })
        return {"status": "done", "scans_averaged": count}

    @app.post("/api/calibration/reset")
    async def reset_calibration():
        cal.reset()
        return {"status": "reset"}

    @app.websocket("/ws")
    async def websocket_endpoint(ws: WebSocket):
        await ws.accept()
        connected_clients.append(ws)
        await ws.send_json({"type": "init", "wavelengths": ds_wavelengths})
        try:
            while True:
                await ws.receive_text()
        except WebSocketDisconnect:
            if ws in connected_clients:
                connected_clients.remove(ws)

    return app


# ---------------------------------------------------------------------------
# HTML page
# ---------------------------------------------------------------------------

HTML_PAGE = """<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Calibration Demo — TECSpec Spectrometer</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: #1a1a2e; color: #e0e0e0; }

  .header { background: #16213e; padding: 10px 20px; display: flex; align-items: center; gap: 12px;
            border-bottom: 1px solid #0f3460; flex-wrap: wrap; }
  .header h1 { font-size: 16px; font-weight: 600; }
  .badge { padding: 3px 10px; border-radius: 10px; font-size: 11px; font-weight: 700; text-transform: uppercase; }
  .badge-dark { background: #333; color: #888; }
  .badge-white { background: #f5a623; color: #000; }
  .badge-substrate { background: #4fc3f7; color: #000; }
  .badge-transition { background: #555; color: #aaa; }
  .badge-ok { background: #43a047; color: #fff; }
  .badge-warn { background: #e53935; color: #fff; }
  .header-right { margin-left: auto; font-size: 11px; color: #555; font-family: monospace; }

  .layout { display: grid; grid-template-columns: 1fr 280px; height: calc(100vh - 45px); }

  /* Charts */
  .charts { padding: 12px 16px; overflow-y: auto; display: flex; flex-direction: column; gap: 4px; }
  .chart-box { flex: 1; min-height: 140px; }
  .chart-header { display: flex; align-items: center; gap: 8px; margin-bottom: 4px; }
  .chart-title { font-size: 12px; font-weight: 600; color: #aaa; }
  .chart-value { font-size: 11px; font-family: monospace; color: #666; }
  canvas { width: 100%; height: calc(100% - 20px); min-height: 120px; background: #0a0a1a; border-radius: 6px; }

  /* Sidebar */
  .sidebar { background: #16213e; padding: 14px; border-left: 1px solid #0f3460; overflow-y: auto;
             display: flex; flex-direction: column; gap: 16px; }
  .section { padding-bottom: 14px; border-bottom: 1px solid #0f3460; }
  .section:last-child { border-bottom: none; }
  .section h3 { font-size: 13px; margin-bottom: 8px; color: #4fc3f7; }

  .btn { display: block; width: 100%; padding: 9px; margin-bottom: 4px; border: none; border-radius: 5px;
         font-size: 12px; font-weight: 600; cursor: pointer; transition: all 0.15s; position: relative; }
  .btn:hover { filter: brightness(1.15); }
  .btn-dark { background: #444; color: #fff; }
  .btn-white { background: #f5a623; color: #000; }
  .btn-reset { background: #c62828; color: #fff; }
  .btn-capturing { outline: 2px solid #4fc3f7; outline-offset: 1px; animation: pulse 0.8s infinite; }
  @keyframes pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.6; } }

  .capture-info { font-size: 11px; color: #4fc3f7; min-height: 16px; margin-bottom: 6px; font-family: monospace; }
  .capture-info.done { color: #43a047; }

  .stat { display: flex; justify-content: space-between; font-size: 11px; padding: 2px 0; }
  .stat-k { color: #666; }
  .stat-v { color: #ccc; font-family: monospace; }

  .formula { font-size: 11px; color: #555; line-height: 1.7; }
  .formula code { color: #aaa; background: #111; padding: 1px 4px; border-radius: 3px; font-size: 11px; }
</style>
</head>
<body>

<div class="header">
  <h1>Calibration Demo</h1>
  <span id="badgeScanType" class="badge badge-dark">DARK</span>
  <span id="badgeCal" class="badge badge-warn">NOT CALIBRATED</span>
  <span class="header-right">Scan <span id="hdrScanNum">0</span> | <span id="hdrMean">—</span></span>
</div>

<div class="layout">
  <div class="charts">
    <div class="chart-box">
      <div class="chart-header">
        <span class="chart-title">Raw Spectrum</span>
        <span class="chart-value" id="rawInfo">—</span>
      </div>
      <canvas id="cRaw"></canvas>
    </div>
    <div class="chart-box">
      <div class="chart-header">
        <span class="chart-title">Dark Reference</span>
        <span class="chart-value" id="darkRefInfo">live preview</span>
      </div>
      <canvas id="cDark"></canvas>
    </div>
    <div class="chart-box">
      <div class="chart-header">
        <span class="chart-title">White / Lamp Reference</span>
        <span class="chart-value" id="whiteRefInfo">live preview</span>
      </div>
      <canvas id="cWhite"></canvas>
    </div>
    <div class="chart-box">
      <div class="chart-header">
        <span class="chart-title">Calibrated Transmittance</span>
        <span class="chart-value" id="calInfo">—</span>
      </div>
      <canvas id="cCal"></canvas>
    </div>
  </div>

  <div class="sidebar">
    <div class="section">
      <h3>Calibration</h3>
      <button id="btnDark" class="btn btn-dark" onclick="toggleDark()">Capture Dark Reference</button>
      <div id="darkCapInfo" class="capture-info"></div>
      <button id="btnWhite" class="btn btn-white" onclick="toggleWhite()">Capture White Reference</button>
      <div id="whiteCapInfo" class="capture-info"></div>
      <button class="btn btn-reset" onclick="doReset()">Reset</button>
    </div>

    <div class="section">
      <h3>Status</h3>
      <div class="stat"><span class="stat-k">Dark ref</span><span class="stat-v" id="sDark">no</span></div>
      <div class="stat"><span class="stat-k">White ref</span><span class="stat-v" id="sWhite">no</span></div>
      <div class="stat"><span class="stat-k">Calibrated</span><span class="stat-v" id="sCal">no</span></div>
      <div class="stat"><span class="stat-k">Scan type</span><span class="stat-v" id="sType">—</span></div>
      <div class="stat"><span class="stat-k">Mean signal</span><span class="stat-v" id="sMean">—</span></div>
      <div class="stat"><span class="stat-k">T% (mean)</span><span class="stat-v" id="sTpct">—</span></div>
    </div>

    <div class="section">
      <h3>How it works</h3>
      <div class="formula">
        <code>T% = (scan - dark) / (white - dark) * 100</code><br><br>
        1. Capture dark ref (shutter closed)<br>
        2. Capture white ref (lamp, no sample)<br>
        3. Every scan is calibrated live<br><br>
        Dark ~0% | White ~100% | Substrate ~87%
      </div>
    </div>
  </div>
</div>

<script>
const C = {
  raw:   document.getElementById('cRaw'),
  dark:  document.getElementById('cDark'),
  white: document.getElementById('cWhite'),
  cal:   document.getElementById('cCal'),
};

let wl = [];  // wavelengths
let scanN = 0;
let capDark = false, capWhite = false;
let hasDark = false, hasWhite = false;
// Store captured references client-side for overlay drawing
let darkRefData = null, whiteRefData = null;

// --- Drawing ---
function draw(canvas, values, color, yMin, yMax, overlay, overlayColor) {
  const ctx = canvas.getContext('2d');
  const dpr = window.devicePixelRatio || 1;
  const r = canvas.getBoundingClientRect();
  canvas.width = r.width * dpr;
  canvas.height = r.height * dpr;
  ctx.scale(dpr, dpr);
  const w = r.width, h = r.height;
  ctx.clearRect(0, 0, w, h);

  // Grid + Y labels
  ctx.strokeStyle = '#1a1a2a';
  ctx.lineWidth = 0.5;
  ctx.fillStyle = '#333';
  ctx.font = '9px monospace';
  for (let i = 0; i <= 4; i++) {
    const y = h * i / 4;
    ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(w, y); ctx.stroke();
    const v = yMax - (yMax - yMin) * i / 4;
    ctx.fillText(v >= 1000 ? (v/1000).toFixed(0) + 'k' : v.toFixed(0), 3, y + 10);
  }

  // X labels
  if (wl.length > 0) {
    ctx.fillStyle = '#2a2a3a';
    ctx.font = '9px monospace';
    for (let nm = 500; nm <= 850; nm += 50) {
      const idx = wl.findIndex(v => v >= nm);
      if (idx >= 0) {
        const x = (idx / (wl.length - 1)) * w;
        ctx.fillText(nm, x - 8, h - 3);
      }
    }
  }

  function drawLine(data, col, lw) {
    if (!data || data.length === 0) return;
    ctx.strokeStyle = col;
    ctx.lineWidth = lw;
    ctx.beginPath();
    for (let i = 0; i < data.length; i++) {
      const x = (i / (data.length - 1)) * w;
      const y = h - ((data[i] - yMin) / (yMax - yMin)) * h;
      i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
    }
    ctx.stroke();
  }

  // Overlay (captured reference) drawn first, dimmer
  if (overlay) drawLine(overlay, overlayColor || 'rgba(255,255,255,0.15)', 1.5);
  // Main line on top
  drawLine(values, color, 1);
}

function drawEmpty(canvas, text) {
  const ctx = canvas.getContext('2d');
  const dpr = window.devicePixelRatio || 1;
  const r = canvas.getBoundingClientRect();
  canvas.width = r.width * dpr;
  canvas.height = r.height * dpr;
  ctx.scale(dpr, dpr);
  ctx.fillStyle = '#333';
  ctx.font = '13px sans-serif';
  ctx.fillText(text, r.width / 2 - 40, r.height / 2);
}

// --- Colors ---
const TYPE_COLOR = { DARK: '#666', WHITE: '#f5a623', SUBSTRATE: '#4fc3f7', TRANSITION: '#888' };
const BADGE_CLASS = { DARK: 'badge-dark', WHITE: 'badge-white', SUBSTRATE: 'badge-substrate', TRANSITION: 'badge-transition' };

// --- WebSocket ---
const ws = new WebSocket('ws://' + location.host + '/ws');

ws.onmessage = (e) => {
  const m = JSON.parse(e.data);

  if (m.type === 'init') {
    wl = m.wavelengths;
    drawEmpty(C.raw, 'Connecting...');
    drawEmpty(C.dark, 'Waiting for dark scans');
    drawEmpty(C.white, 'Waiting for white scans');
    drawEmpty(C.cal, 'Capture dark + white first');
    return;
  }

  if (m.type === 'dark_ref') {
    darkRefData = m.values;
    document.getElementById('darkRefInfo').textContent = 'captured (averaged)';
    document.getElementById('darkRefInfo').style.color = '#43a047';
    return;
  }
  if (m.type === 'white_ref') {
    whiteRefData = m.values;
    document.getElementById('whiteRefInfo').textContent = 'captured (averaged)';
    document.getElementById('whiteRefInfo').style.color = '#43a047';
    return;
  }

  if (m.type !== 'scan') return;

  scanN++;
  const st = m.scan_type;
  const col = TYPE_COLOR[st] || '#fff';

  // Header
  document.getElementById('hdrScanNum').textContent = scanN;
  document.getElementById('hdrMean').textContent = 'mean: ' + m.mean.toFixed(0);
  const b = document.getElementById('badgeScanType');
  b.textContent = st;
  b.className = 'badge ' + (BADGE_CLASS[st] || 'badge-dark');

  // Stats
  document.getElementById('sType').textContent = st;
  document.getElementById('sMean').textContent = m.mean.toFixed(0);

  // Raw chart — auto-range
  let yMin = -100, yMax = 100;
  if (st === 'WHITE' || st === 'TRANSITION') { yMin = 0; yMax = 60000; }
  else if (st === 'SUBSTRATE') { yMin = 0; yMax = 55000; }
  draw(C.raw, m.values, col, yMin, yMax);
  document.getElementById('rawInfo').textContent = st + ' | mean ' + m.mean.toFixed(0);

  // Dark chart — update live on DARK scans, show captured ref as overlay
  if (st === 'DARK') {
    draw(C.dark, m.values, '#888', -100, 100, darkRefData, 'rgba(67,160,71,0.5)');
  }

  // White chart — update live on WHITE scans, show captured ref as overlay
  if (st === 'WHITE') {
    draw(C.white, m.values, '#f5a623', 0, 60000, whiteRefData, 'rgba(67,160,71,0.5)');
  }

  // Calibrated chart — only update for SUBSTRATE scans (hold last result)
  if (m.calibrated && st === 'SUBSTRATE') {
    draw(C.cal, m.calibrated, '#4fc3f7', 0, 100);
    const pct = m.calibrated_mean.toFixed(1) + '%';
    document.getElementById('calInfo').textContent = 'T%=' + pct;
    document.getElementById('sTpct').textContent = pct;
  }

  // Calibration state
  const cal = m.calibration;
  hasDark = cal.has_dark;
  hasWhite = cal.has_white;
  capDark = cal.capturing_dark;
  capWhite = cal.capturing_white;

  document.getElementById('sDark').textContent = hasDark ? 'yes' : 'no';
  document.getElementById('sDark').style.color = hasDark ? '#43a047' : '#888';
  document.getElementById('sWhite').textContent = hasWhite ? 'yes' : 'no';
  document.getElementById('sWhite').style.color = hasWhite ? '#43a047' : '#888';
  document.getElementById('sCal').textContent = cal.is_calibrated ? 'yes' : 'no';
  document.getElementById('sCal').style.color = cal.is_calibrated ? '#43a047' : '#888';

  const bc = document.getElementById('badgeCal');
  bc.textContent = cal.is_calibrated ? 'CALIBRATED' : 'NOT CALIBRATED';
  bc.className = 'badge ' + (cal.is_calibrated ? 'badge-ok' : 'badge-warn');

  // Buttons
  const bd = document.getElementById('btnDark');
  bd.textContent = capDark ? 'Stop Dark Capture' : (hasDark ? 'Recapture Dark' : 'Capture Dark Reference');
  bd.classList.toggle('btn-capturing', capDark);

  const bw = document.getElementById('btnWhite');
  bw.textContent = capWhite ? 'Stop White Capture' : (hasWhite ? 'Recapture White' : 'Capture White Reference');
  bw.classList.toggle('btn-capturing', capWhite);

  // Capture counters
  if (capDark) document.getElementById('darkCapInfo').textContent = 'collecting... ' + cal.dark_scans_collected + ' scans';
  if (capWhite) document.getElementById('whiteCapInfo').textContent = 'collecting... ' + cal.white_scans_collected + ' scans';
};

// --- Actions ---
async function toggleDark() {
  if (capDark) {
    const r = await (await fetch('/api/calibration/dark/stop', {method:'POST'})).json();
    const el = document.getElementById('darkCapInfo');
    el.textContent = 'averaged ' + r.scans_averaged + ' scans';
    el.className = 'capture-info done';
  } else {
    await fetch('/api/calibration/dark/start', {method:'POST'});
    document.getElementById('darkCapInfo').textContent = 'collecting...';
    document.getElementById('darkCapInfo').className = 'capture-info';
  }
}

async function toggleWhite() {
  if (capWhite) {
    const r = await (await fetch('/api/calibration/white/stop', {method:'POST'})).json();
    const el = document.getElementById('whiteCapInfo');
    el.textContent = 'averaged ' + r.scans_averaged + ' scans';
    el.className = 'capture-info done';
  } else {
    await fetch('/api/calibration/white/start', {method:'POST'});
    document.getElementById('whiteCapInfo').textContent = 'collecting...';
    document.getElementById('whiteCapInfo').className = 'capture-info';
  }
}

async function doReset() {
  await fetch('/api/calibration/reset', {method:'POST'});
  darkRefData = null; whiteRefData = null;
  document.getElementById('darkCapInfo').textContent = '';
  document.getElementById('whiteCapInfo').textContent = '';
  document.getElementById('sTpct').textContent = '—';
  document.getElementById('calInfo').textContent = '—';
  document.getElementById('darkRefInfo').textContent = 'live preview';
  document.getElementById('darkRefInfo').style.color = '';
  document.getElementById('whiteRefInfo').textContent = 'live preview';
  document.getElementById('whiteRefInfo').style.color = '';
  drawEmpty(C.cal, 'Capture dark + white first');
}
</script>

</body>
</html>
"""


def main() -> None:
    parser = argparse.ArgumentParser(description="Calibration demo — web UI with real spectrometer data")
    parser.add_argument(
        "csv_path",
        nargs="?",
        default=str(Path(__file__).parent / "data" / "raw_spectra.csv"),
        help="Path to raw_spectra.csv",
    )
    parser.add_argument("--port", type=int, default=8050, help="Port for web UI (default: 8050)")
    parser.add_argument("--speed", type=float, default=5.0, help="Replay speed multiplier (default: 5)")

    args = parser.parse_args()

    csv_path = Path(args.csv_path)
    if not csv_path.exists():
        print(f"ERROR: {csv_path} not found")
        sys.exit(1)

    app = create_app(csv_path, replay_speed=args.speed)

    logger.info(f"Starting calibration demo on http://localhost:{args.port}")
    logger.info(f"Replaying {csv_path} at {args.speed}x speed")
    logger.info("Open your browser to see the live spectra and calibration controls")

    uvicorn.run(app, host="0.0.0.0", port=args.port)


if __name__ == "__main__":
    main()
