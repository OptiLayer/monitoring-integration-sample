"""HORIBA Monochromator integration test script.

Tests monochromator functionality via ICL:
1. Connect to ICL, discover monochromator
2. Initialize monochromator, read configuration
3. Set wavelength
4. Grating switch test (optional)

Prerequisites:
  - HORIBA ICL.exe installed, licensed, and running
  - pip install horiba-sdk

Usage:
  horiba-test.exe [--icl-ip 127.0.0.1] [--icl-port 25010]
                  [--start-icl] [--wavelength 500]
"""

from __future__ import annotations

import argparse
import asyncio
import time
from typing import Any

from horiba_sdk.devices.device_manager import DeviceManager
from horiba_sdk.devices.single_devices.monochromator import Monochromator

# ---------------------------------------------------------------------------
# Monkey-patch: horiba_sdk 1.0.0 crashes with KeyError: 'devices' when
# discovery returns an empty response.  Patch all three to be safe.
# ---------------------------------------------------------------------------
from horiba_sdk.devices.ccd_discovery import ChargeCoupledDevicesDiscovery
from horiba_sdk.devices.monochromator_discovery import MonochromatorsDiscovery
from horiba_sdk.devices.spectracq3_discovery import SpectrAcq3Discovery


def _safe_parse_monos(
    self: MonochromatorsDiscovery,
    raw_device_list: dict[str, Any],
) -> list:
    from horiba_sdk.devices.single_devices import Monochromator as _Mono

    print(f"  mono_list response: {raw_device_list}")
    detected: list[_Mono] = []
    for device in raw_device_list.get("devices", []):
        print(f"    Mono device: {device}")
        try:
            mono = _Mono(device["index"], self._communicator, self._error_db)
            print(f"    -> Detected Monochromator: {device['deviceType']}")
            detected.append(mono)
        except Exception as e:
            print(f"    -> Error parsing Monochromator: {e}")
    return detected


def _safe_parse_ccds(self: Any, raw_device_list: dict[str, Any]) -> list:
    return []


def _safe_parse_spectracq3(self: Any, raw_device_list: dict[str, Any]) -> list:
    return []


MonochromatorsDiscovery._parse_monos = _safe_parse_monos  # type: ignore[assignment]
ChargeCoupledDevicesDiscovery._parse_ccds = _safe_parse_ccds  # type: ignore[assignment]
SpectrAcq3Discovery._parse_devices = _safe_parse_spectracq3  # type: ignore[assignment]


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


async def wait_mono_ready(mono: Monochromator, timeout: float = 120.0) -> None:
    """Poll monochromator until not busy, with timeout."""
    t0 = time.monotonic()
    while await mono.is_busy():
        if time.monotonic() - t0 > timeout:
            raise TimeoutError(f"Monochromator still busy after {timeout}s")
        await asyncio.sleep(0.5)


# ---------------------------------------------------------------------------
# Test steps
# ---------------------------------------------------------------------------


async def test_connect_and_discover(args: argparse.Namespace) -> DeviceManager:
    """Step 1: Connect to ICL and discover devices."""
    print("=" * 60)
    print("STEP 1: Connect to ICL and discover devices")
    print("=" * 60)

    dm = DeviceManager(
        start_icl=args.start_icl,
        icl_ip=args.icl_ip,
        icl_port=str(args.icl_port),
        enable_binary_messages=True,
    )
    await dm.start()

    monos = dm.monochromators
    print(f"  Monochromators found: {len(monos)}")
    for i, m in enumerate(monos):
        print(f"    [{i}] id={m.id()}, type={type(m).__name__}")

    if len(monos) == 0:
        raise RuntimeError("No monochromator found. Check USB connection and ICL.")

    return dm


async def test_initialize_mono(mono: Monochromator) -> dict:
    """Step 2: Initialize monochromator, read configuration."""
    print("\n" + "=" * 60)
    print("STEP 2: Initialize monochromator")
    print("=" * 60)

    await mono.open()

    if not await mono.is_initialized():
        print("  Initializing (homing)... this may take a minute")
        await mono.initialize()
        await wait_mono_ready(mono, timeout=180.0)
    print("  Monochromator initialized")

    config = await mono.configuration()
    print("  Configuration:")
    for key, val in config.items():
        print(f"    {key}: {val}")

    wl = await mono.get_current_wavelength()
    grating = await mono.get_turret_grating()
    print(f"  Current wavelength: {wl:.2f} nm")
    print(f"  Current grating:    {grating}")

    # List available gratings
    print("  Available gratings:")
    for g in Monochromator.Grating:
        try:
            print(f"    {g.name}: {g.value}")
        except Exception:
            pass

    # Read slit positions
    for slit in [Monochromator.Slit.A, Monochromator.Slit.B]:
        try:
            pos = await mono.get_slit_position_in_mm(slit)
            print(f"  Slit {slit.name} position:  {pos:.3f} mm")
        except Exception as e:
            print(f"  Slit {slit.name}: not available ({e})")

    # Read shutter status
    try:
        shutter_pos = await mono.get_shutter_position(Monochromator.Shutter.FIRST)
        print(f"  Shutter status:     {shutter_pos}")
    except Exception as e:
        print(f"  Shutter: not available ({e})")

    # Read mirror positions
    for mirror in [Monochromator.Mirror.ENTRANCE, Monochromator.Mirror.EXIT]:
        try:
            pos = await mono.get_mirror_position(mirror)
            print(f"  Mirror {mirror.name}: {pos}")
        except Exception as e:
            print(f"  Mirror {mirror.name}: not available ({e})")

    return config


async def test_set_wavelength(mono: Monochromator, wavelength: float) -> float:
    """Step 3: Move monochromator to target wavelength."""
    print("\n" + "=" * 60)
    print(f"STEP 3: Move to {wavelength:.1f} nm")
    print("=" * 60)

    t0 = time.monotonic()
    await mono.move_to_target_wavelength(wavelength)
    await wait_mono_ready(mono)
    elapsed = time.monotonic() - t0

    actual_wl = await mono.get_current_wavelength()
    print(f"  Requested:  {wavelength:.1f} nm")
    print(f"  Actual:     {actual_wl:.2f} nm")
    print(f"  Move time:  {elapsed:.1f} s")

    return actual_wl


async def test_grating_switch(mono: Monochromator) -> None:
    """Step 4: Test grating switching."""
    print("\n" + "=" * 60)
    print("STEP 4: Grating switch test")
    print("=" * 60)

    current = await mono.get_turret_grating()
    print(f"  Current grating: {current}")

    # Try switching to each grating and back
    for grating in Monochromator.Grating:
        if grating == current:
            continue
        try:
            print(f"  Switching to grating {grating.name}...")
            t0 = time.monotonic()
            await mono.set_turret_grating(grating)
            await wait_mono_ready(mono, timeout=120.0)
            elapsed = time.monotonic() - t0
            print(f"  Switched to {grating.name} in {elapsed:.1f}s")
            break  # only test one switch
        except Exception as e:
            print(f"  Grating {grating.name} not available: {e}")

    # Switch back
    print(f"  Switching back to {current.name}...")
    await mono.set_turret_grating(current)
    await wait_mono_ready(mono)
    print(f"  Restored grating {current.name}")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


async def run_all_tests(args: argparse.Namespace) -> None:
    dm = await test_connect_and_discover(args)
    mono = dm.monochromators[0]

    results: list[tuple[str, str]] = [("1. Connect to ICL", "PASS")]

    try:
        # Step 2: Init monochromator
        try:
            await test_initialize_mono(mono)
            results.append(("2. Init monochromator", "PASS"))
        except Exception as e:
            results.append(("2. Init monochromator", f"FAIL: {e}"))
            print(f"\n  ** Monochromator init failed: {e}")
            return

        # Step 3: Move to wavelength
        try:
            await test_set_wavelength(mono, args.wavelength)
            results.append(("3. Set wavelength", "PASS"))
        except Exception as e:
            results.append(("3. Set wavelength", f"FAIL: {e}"))
            print(f"\n  ** Wavelength move failed: {e}")

        # Step 4: Grating switch (opt-in)
        if args.test_grating:
            try:
                await test_grating_switch(mono)
                results.append(("4. Grating switch", "PASS"))
            except Exception as e:
                results.append(("4. Grating switch", f"FAIL: {e}"))
                print(f"\n  ** Grating switch failed: {e}")

    finally:
        print("\nCleaning up...")
        try:
            await mono.close()
        except Exception as e:
            print(f"  Mono close error: {e}")
        await dm.stop()

    # Print summary
    print("\n" + "=" * 60)
    print("TEST RESULTS SUMMARY")
    print("=" * 60)
    n_pass = sum(1 for _, s in results if s == "PASS")
    n_fail = sum(1 for _, s in results if s.startswith("FAIL"))
    for name, status in results:
        print(f"  {name}: {status}")
    print(f"\n  {n_pass} passed, {n_fail} failed")
    if n_fail == 0:
        print("\n  ALL TESTS PASSED")
    print("=" * 60)


def main():
    parser = argparse.ArgumentParser(
        description="HORIBA Monochromator integration test (iHR320, iHR550, etc.)",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument("--icl-ip", default="127.0.0.1", help="ICL WebSocket IP")
    parser.add_argument(
        "--icl-port", type=int, default=25010, help="ICL WebSocket port"
    )
    parser.add_argument(
        "--start-icl", action="store_true", help="Auto-start ICL.exe (default: don't start)"
    )
    parser.add_argument(
        "--wavelength", type=float, default=500.0, help="Target wavelength in nm"
    )
    parser.add_argument(
        "--test-grating", action="store_true", help="Test grating switching (slow)"
    )

    args = parser.parse_args()

    try:
        asyncio.run(run_all_tests(args))
    except ConnectionRefusedError:
        print(f"\nERROR: Cannot connect to ICL at {args.icl_ip}:{args.icl_port}")
        print("Make sure ICL.exe is running, or use --start-icl to auto-start it.")
    except KeyboardInterrupt:
        print("\nInterrupted by user.")
    except Exception as e:
        print(f"\nERROR: {e}")
        raise


if __name__ == "__main__":
    main()
