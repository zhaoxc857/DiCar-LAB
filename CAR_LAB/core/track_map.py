"""Dead-reckoning track map from speed + yaw rate telemetry.

Units: `speed` carries the vehicle's own speed unit (MCU-defined); multiply
by `meters_per_unit` to get metres. `yaw_rate` is degrees/second (the same
convention the corner analyzer uses). Drift is inherent to dead reckoning;
the UI exposes the scale factor so users can calibrate against a known lap.
"""

from __future__ import annotations

import math

DEFAULT_SPEED_KEY = "speed"
DEFAULT_YAW_RATE_KEY = "gyro_z"
MAX_STEP_S = 0.5  # 忽略超过 0.5s 的采样间隔（断流时不产生大跳变）


class TrackMapIntegrator:
    """Incremental heading/position integration for one continuous run."""

    def __init__(self, meters_per_unit: float = 1.0,
                 speed_key: str = DEFAULT_SPEED_KEY,
                 yaw_rate_key: str = DEFAULT_YAW_RATE_KEY):
        self.meters_per_unit = float(meters_per_unit)
        self.speed_key = speed_key
        self.yaw_rate_key = yaw_rate_key
        self.reset()

    def reset(self):
        self.x = 0.0
        self.y = 0.0
        self.heading_deg = 0.0
        self.last_t = None

    def update(self, sample: dict, t: float | None = None):
        """Feed one telemetry sample; returns the current (x, y)."""
        if t is None:
            t = sample.get("t")
        dt = None
        if self.last_t is not None and t is not None:
            dt = float(t) - self.last_t
            if dt <= 0.0 or dt > MAX_STEP_S:
                dt = None
        self.last_t = t

        speed = sample.get(self.speed_key)
        yaw_rate = sample.get(self.yaw_rate_key)
        if _num(yaw_rate):
            self.heading_deg = (self.heading_deg + float(yaw_rate) * (dt or 0.0)) % 360.0
        if _num(speed) and dt:
            rad = math.radians(self.heading_deg)
            distance = float(speed) * self.meters_per_unit * dt
            self.x += distance * math.sin(rad)
            self.y += distance * math.cos(rad)
        return (self.x, self.y)


def reconstruct_path(samples: list, meters_per_unit: float = 1.0,
                     speed_key: str = DEFAULT_SPEED_KEY,
                     yaw_rate_key: str = DEFAULT_YAW_RATE_KEY) -> list:
    """Batch reconstruction: list of samples (with optional "t") -> [(x, y)]."""
    integrator = TrackMapIntegrator(meters_per_unit, speed_key, yaw_rate_key)
    points = []
    for sample in samples:
        points.append(integrator.update(sample))
    return points


def _num(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool)
