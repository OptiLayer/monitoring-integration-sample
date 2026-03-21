"""Tests for calibration logic."""

from pathlib import Path

import numpy as np
import pytest

from horiba_service.calibration import CalibrationState


def test_not_calibrated_initially():
    cal = CalibrationState()
    assert not cal.is_calibrated
    assert cal.status_dict()["has_dark"] is False
    assert cal.status_dict()["has_white"] is False


def test_calibrate_returns_zeros_when_not_calibrated():
    cal = CalibrationState()
    scan = np.array([100.0, 200.0, 300.0])
    result = cal.calibrate(scan)
    np.testing.assert_array_equal(result, [0.0, 0.0, 0.0])


def test_dark_capture():
    cal = CalibrationState()
    cal.capturing_dark = True

    cal.process_scan(np.array([10.0, 20.0, 30.0]))
    cal.process_scan(np.array([12.0, 22.0, 32.0]))
    assert cal.status_dict()["dark_scans_collected"] == 2

    count = cal.finalize_dark()
    assert count == 2
    assert cal.dark_ref is not None
    np.testing.assert_allclose(cal.dark_ref, [11.0, 21.0, 31.0])


def test_white_capture():
    cal = CalibrationState()
    cal.capturing_white = True

    cal.process_scan(np.array([1000.0, 2000.0, 3000.0]))
    count = cal.finalize_white()
    assert count == 1
    np.testing.assert_allclose(cal.white_ref, [1000.0, 2000.0, 3000.0])


def test_calibration_formula():
    cal = CalibrationState()
    cal.dark_ref = np.array([0.0, 100.0, 200.0])
    cal.white_ref = np.array([1000.0, 1000.0, 1000.0])

    # Substrate scan
    scan = np.array([870.0, 820.0, 760.0])
    result = cal.calibrate(scan)

    # T% = (scan - dark) / (white - dark) * 100
    np.testing.assert_allclose(result, [87.0, 80.0, 70.0])


def test_calibration_clips_to_0_100():
    cal = CalibrationState()
    cal.dark_ref = np.array([0.0, 0.0])
    cal.white_ref = np.array([1000.0, 1000.0])

    # Values outside range
    scan = np.array([-50.0, 1200.0])
    result = cal.calibrate(scan)
    np.testing.assert_allclose(result, [0.0, 100.0])


def test_calibration_handles_zero_denominator():
    cal = CalibrationState()
    cal.dark_ref = np.array([500.0, 500.0])
    cal.white_ref = np.array([500.0, 1000.0])  # First pixel: white == dark

    scan = np.array([500.0, 750.0])
    result = cal.calibrate(scan)
    # First pixel: denominator clamped to 1.0, result = 0/1*100 = 0, clipped
    assert result[0] >= 0.0
    assert result[1] == pytest.approx(50.0)


def test_reset():
    cal = CalibrationState()
    cal.dark_ref = np.array([0.0])
    cal.white_ref = np.array([1000.0])
    assert cal.is_calibrated

    cal.reset()
    assert not cal.is_calibrated
    assert cal.dark_ref is None
    assert cal.white_ref is None


def test_capture_only_accumulates_while_active():
    cal = CalibrationState()

    # Not capturing — scan should not accumulate
    cal.process_scan(np.array([100.0]))
    assert len(cal.dark_accumulator) == 0

    # Start capturing
    cal.capturing_dark = True
    cal.process_scan(np.array([100.0]))
    assert len(cal.dark_accumulator) == 1

    # Stop capturing
    cal.finalize_dark()
    cal.process_scan(np.array([100.0]))
    assert len(cal.dark_accumulator) == 0


def test_save_and_load(tmp_path):
    path = tmp_path / "cal.json"

    # Save
    cal = CalibrationState()
    cal.dark_ref = np.array([10.0, 20.0, 30.0])
    cal.white_ref = np.array([1000.0, 2000.0, 3000.0])
    cal.save(path)
    assert path.exists()

    # Load into fresh state
    cal2 = CalibrationState()
    assert not cal2.is_calibrated
    loaded = cal2.load(path)
    assert loaded
    assert cal2.is_calibrated
    np.testing.assert_allclose(cal2.dark_ref, [10.0, 20.0, 30.0])
    np.testing.assert_allclose(cal2.white_ref, [1000.0, 2000.0, 3000.0])


def test_load_nonexistent_returns_false(tmp_path):
    cal = CalibrationState()
    assert not cal.load(tmp_path / "nope.json")
    assert not cal.is_calibrated


def test_save_partial(tmp_path):
    path = tmp_path / "cal.json"

    # Save with only dark
    cal = CalibrationState()
    cal.dark_ref = np.array([5.0, 10.0])
    cal.save(path)

    cal2 = CalibrationState()
    cal2.load(path)
    assert cal2.dark_ref is not None
    assert cal2.white_ref is None
    assert not cal2.is_calibrated
