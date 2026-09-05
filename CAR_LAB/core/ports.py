"""Enumerate host serial ports for UI dropdowns (Qt-free)."""

from __future__ import annotations


def list_serial_ports() -> list:
    """Return sorted COM port device names; empty list when pyserial is
    missing or enumeration fails (UI falls back to free-text input)."""
    try:
        from serial.tools import list_ports
    except Exception:
        return []
    devices = set()
    for port in list_ports.comports():
        device = (getattr(port, "device", "") or "").strip()
        if device:
            devices.add(device)
    return sorted(devices)
