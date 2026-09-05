"""Derived oscilloscope channels computed client-side from telemetry dicts.

Derived keys are prefixed with "@" so they never collide with MCU fields;
they flow through the same storage/export path as raw channels.
"""

from __future__ import annotations


def _num(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def derive_channels(data: dict, config: dict) -> dict:
    """Return {"@key": value} derived channels available in this sample."""
    out = {}
    lab_cfg = (config or {}).get("speed_lab", {}) or {}
    err_key = str(lab_cfg.get("error_key", "speed_error"))
    err = data.get(err_key)
    if _num(err):
        out["@err_sq"] = float(err) ** 2

    battery = data.get("battery")
    currents = [float(v) for k, v in data.items()
                if str(k).endswith("_current") and _num(v)]
    if _num(battery) and currents:
        out["@power_w"] = float(battery) * sum(currents)
    return out


DERIVED_CHANNELS = {
    "@err_sq": ("误差平方", "Error Squared"),
    "@power_w": ("电功率 W", "Electrical Power"),
}

DERIVED_PRESET = {"派生": ["@err_sq", "@power_w"]}
