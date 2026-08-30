"""Resolve read-only resource and writable data directories for every run mode.

Source runs keep everything under the repository CAR_LAB directory. Frozen
runs (PyInstaller one-folder or one-file) read bundled resources from
sys._MEIPASS and keep user-writable files under %LOCALAPPDATA%/DiCAR LAB,
because the one-file bootloader wipes its temp extraction directory on exit
and the one-folder _internal directory is replaced on every app upgrade.
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

APP_DATA_DIRNAME = "DiCAR LAB"


def is_frozen() -> bool:
    return bool(getattr(sys, "frozen", False))


def resource_root() -> Path:
    """Read-only bundled resources: vehicles/, docs/, examples/, tools/."""
    meipass = getattr(sys, "_MEIPASS", None)
    if meipass:
        return Path(meipass)
    return Path(__file__).resolve().parents[1]


def data_root() -> Path:
    """Writable per-user directory: data/, profiles/, reports/, logs/."""
    if is_frozen():
        base = os.environ.get("LOCALAPPDATA")
        root = Path(base) if base else Path.home()
        return root / APP_DATA_DIRNAME
    return Path(__file__).resolve().parents[1]
