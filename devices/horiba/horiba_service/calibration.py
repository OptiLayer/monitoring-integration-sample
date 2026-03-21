"""Calibration state for broadband spectrometer — dark/white reference capture."""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np
import numpy.typing as npt


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
        """Convert raw scan to transmittance (0-100%)."""
        if not self.is_calibrated:
            return np.zeros_like(scan)
        denominator = self.white_ref - self.dark_ref
        denominator = np.where(np.abs(denominator) < 1.0, 1.0, denominator)
        result = (scan - self.dark_ref) / denominator * 100.0
        return np.clip(result, 0.0, 100.0)

    def process_scan(self, scan: npt.NDArray[np.float64]) -> None:
        """Accumulate scan if capturing is active."""
        if self.capturing_dark:
            self.dark_accumulator.append(scan.copy())
        if self.capturing_white:
            self.white_accumulator.append(scan.copy())

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
