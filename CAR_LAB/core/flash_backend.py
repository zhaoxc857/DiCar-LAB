"""Locate the stm32flash tool and build the wireless flash command line.

The wireless flashing recipe for the STM32F103 line car over an HC-05
Bluetooth serial module is exactly the one validated by hand:

    stm32flash.exe -b <baud> -w <firmware> -v -g 0x0 <port>

The factory bootloader auto-detects the baud rate, the HC-05 must be
configured to even parity (8E1) to match it, and BOOT0 must be strapped
high before powering the car. Keeping the command in one function makes
the GUI flow testable without a real device.
"""

import sys
from pathlib import Path


def find_stm32flash(base_dir=None) -> str | None:
    """Return the bundled stm32flash.exe path, or None when absent.

    Search order: explicit base, PyInstaller bundle (sys._MEIPASS), the
    repository tools directory next to this package.
    """
    candidates = []
    if base_dir:
        candidates.append(Path(base_dir) / "tools" / "stm32flash.exe")
    meipass = getattr(sys, "_MEIPASS", None)
    if meipass:
        candidates.append(Path(meipass) / "tools" / "stm32flash.exe")
    candidates.append(
        Path(__file__).resolve().parents[2] / "tools" / "stm32flash.exe"
    )
    for candidate in candidates:
        if candidate.is_file():
            return str(candidate)
    return None


def build_flash_command(exe: str, port: str, baud: int, firmware: str) -> list:
    return [
        str(exe),
        "-b", str(int(baud)),
        "-w", str(firmware),
        "-v",
        "-g", "0x0",
        str(port),
    ]
