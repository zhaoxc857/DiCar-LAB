"""Locate the stm32flash tool, size-limit firmware, and build/parse its output.

The wireless flashing recipe for the STM32 line over an HC-05 Bluetooth
serial module is exactly the one validated by hand:

    stm32flash.exe -m 8e1 -b <baud> -w <firmware> -v -g 0x0 <port>

The factory bootloader requires even parity (AN3155, 8E1); the PC-side
serial format is now passed explicitly with -m instead of relying on the
user to have configured a matching HC-05 default. BOOT0 must still be
strapped high before powering the car. Keeping the command in one
function makes the GUI flow testable without a real device.

stm32flash reports write progress as repeated carriage-return lines
"Wrote [and verified ]address 0x... (NN.NN%)" on stdout, and sends error
text to stderr; classify_output_segment() separates progress from log
lines so the GUI can drive a progress bar instead of flooding the log.
"""

import re
import sys
from pathlib import Path

# PC-side serial format passed to stm32flash -m. AN3155 bootloaders use
# even parity, so 8e1 is the default; 8n1 is offered for diagnostics.
DEFAULT_SERIAL_MODE = "8e1"

# Conservative per-family firmware size caps. They exist to reject obviously
# wrong files before touching the serial port; real devices report their
# exact flash size during the bootloader handshake.
FAMILY_FLASH_SIZE_LIMITS = {
    "STM32F1": 1024 * 1024,
    "STM32F4": 2 * 1024 * 1024,
    "MSPM0G3507": 128 * 1024,
}

WROTE_PROGRESS_RE = re.compile(
    r"^Wrote(?: and verified)? address 0x[0-9A-Fa-f]+ \((\d+(?:\.\d+)?)%\)\s*$"
)


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


def build_flash_command(exe: str, port: str, baud: int, firmware: str,
                        serial_mode: str = DEFAULT_SERIAL_MODE) -> list:
    command = [str(exe)]
    if serial_mode:
        command += ["-m", str(serial_mode)]
    command += [
        "-b", str(int(baud)),
        "-w", str(firmware),
        "-v",
        "-g", "0x0",
        str(port),
    ]
    return command


def firmware_size_limit(family: str) -> int:
    """User-flash size cap in bytes, 0 when the family has no known cap."""
    return FAMILY_FLASH_SIZE_LIMITS.get(family, 0)


def check_firmware_size(family: str, size: int) -> str | None:
    """Return a user-facing rejection message, or None when acceptable."""
    limit = firmware_size_limit(family)
    if limit and size > limit:
        return f"固件 {size} 字节超出 {family} 主闪存上限 {limit // 1024}KB，无法烧录"
    return None


def split_output_segments(text: str) -> list:
    """Split console output on \\r/\\n into candidate segments."""
    return re.split(r"[\r\n]+", text)


def classify_output_segment(segment: str) -> tuple:
    """Return ("progress", percent) for write-progress lines, else ("log", None)."""
    match = WROTE_PROGRESS_RE.match(segment.strip())
    if match:
        return ("progress", float(match.group(1)))
    return ("log", None)
