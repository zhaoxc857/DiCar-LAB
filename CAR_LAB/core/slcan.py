"""SLCAN (Lawicel CAN-USB style) ASCII frame codec - the wire format for
the future CAN transport over serial CAN adapters.

A frame line is: 't' + 3 hex id + 1 hex DLC + data (standard),
'T' + 8 hex id + ... (extended), lower-case 'r'/'R' for remote frames.
This module is the pure codec only; end-to-end CAN needs an MCU-side
bridge firmware (see docs/CAN_接入设计.md) and is intentionally not wired
into TransportManager yet.
"""

from __future__ import annotations

import re

FRAME_RE = re.compile(r"^([tTrR])([0-9A-Fa-f]{3})([0-4])([0-9A-Fa-f]*)$")
EXTENDED_RE = re.compile(r"^([TR])([0-9A-Fa-f]{8})([0-8])([0-9A-Fa-f]*)$")


class CanFrame:
    __slots__ = ("can_id", "data", "extended", "remote")

    def __init__(self, can_id: int, data: bytes = b"", extended: bool = False,
                 remote: bool = False):
        self.can_id = int(can_id)
        self.data = bytes(data)
        self.extended = bool(extended)
        self.remote = bool(remote)

    def __eq__(self, other):
        return (isinstance(other, CanFrame)
                and (self.can_id, self.data, self.extended, self.remote)
                == (other.can_id, other.data, other.extended, other.remote))

    def __repr__(self):
        return (f"CanFrame(0x{self.can_id:X}, {self.data!r}, "
                f"extended={self.extended}, remote={self.remote})")


def encode_frame(frame: CanFrame) -> str:
    """Encode one frame into a SLCAN ASCII line (no trailing CR).

    SLCAN prefixes: t=standard data, r=standard remote, T=extended data,
    R=extended remote.
    """
    if frame.extended:
        prefix = "R" if frame.remote else "T"
    else:
        prefix = "r" if frame.remote else "t"
    width = 8 if frame.extended else 3
    dlc = 0 if frame.remote else len(frame.data)
    return f"{prefix}{frame.can_id:0{width}X}{dlc:X}" + frame.data.hex().upper()


def parse_line(line: str):
    """Parse one SLCAN line; returns CanFrame, or None for non-frame lines
    (CR/LF, commands like 'O'/'C', status like OK/NOPE..., malformed frames)."""
    text = line.strip().rstrip("\r")
    if not text:
        return None
    extended = text[0] in ("T", "R")
    remote = text[0] in ("r", "R")
    match = (EXTENDED_RE if extended else FRAME_RE).match(text)
    if match is None:
        return None
    _cmd, id_hex, dlc_hex, data_hex = match.groups()
    dlc = int(dlc_hex)
    try:
        data = bytes.fromhex(data_hex) if data_hex else b""
    except ValueError:
        return None
    expected = 0 if remote else dlc
    if len(data) != expected:
        return None
    return CanFrame(int(id_hex, 16), data, extended=extended, remote=remote)
