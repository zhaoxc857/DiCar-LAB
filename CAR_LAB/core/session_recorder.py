"""Session recording and replay: JSONL files of {"t": seconds, "d": {...}}
lines under data_root()/recordings/, re-emitted on the telemetry bus so
every page (scope, corner analysis, diagnostics) runs against past data.
"""

from __future__ import annotations

import json
import time
from pathlib import Path

from PySide6.QtCore import QObject, QTimer, Signal

from core.paths import data_root

RECORDINGS_DIR = data_root() / "recordings"
MAX_LINES = 200_000  # 约 60MB 上限，达到后自动停止


def recordings_dir() -> Path:
    RECORDINGS_DIR.mkdir(parents=True, exist_ok=True)
    return RECORDINGS_DIR


class SessionRecorder(QObject):
    """Subscribe-and-write recorder; one JSONL file per session."""

    stopped = Signal(str, int)  # path, lines

    def __init__(self, bus, parent=None):
        super().__init__(parent)
        self.bus = bus
        self._path = None
        self._file = None
        self._count = 0
        self._t0 = 0.0

    @property
    def recording(self) -> bool:
        return self._file is not None

    @property
    def count(self) -> int:
        return self._count

    def start(self) -> Path:
        if self._file is not None:
            return self._path
        recordings_dir()
        self._path = recordings_dir() / f"session_{time.strftime('%Y%m%d_%H%M%S')}.jsonl"
        self._file = open(self._path, "w", encoding="utf-8")
        self._count = 0
        self._t0 = time.monotonic()
        self.bus.telemetry.connect(self._tel)
        return self._path

    def stop(self):
        if self._file is None:
            return None
        self.bus.telemetry.disconnect(self._tel)
        path, self._path = self._path, None
        self._file.close()
        self._file = None
        self.stopped.emit(str(path), self._count)
        return path

    def _tel(self, data: dict):
        if self._file is None:
            return
        self._file.write(json.dumps(
            {"t": round(time.monotonic() - self._t0, 4), "d": data},
            ensure_ascii=False, separators=(",", ":"),
        ) + "\n")
        self._count += 1
        if self._count >= MAX_LINES:
            self.stop()


def load_recording(path) -> list:
    """Read a JSONL recording into [(t, dict)] sorted by t."""
    samples = []
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            t = obj.get("t")
            data = obj.get("d")
            if isinstance(t, (int, float)) and isinstance(data, dict):
                samples.append((float(t), data))
    samples.sort(key=lambda item: item[0])
    return samples


def samples_due(samples: list, cursor: int, elapsed: float, speed: float = 1.0):
    """Pure scheduler helper: indices of samples due by replay-time.

    Returns (new_cursor, [sample_dict...]). `elapsed` is wall seconds since
    play started; replay-time = elapsed * speed.
    """
    due = []
    while cursor < len(samples) and samples[cursor][0] <= elapsed * speed:
        due.append(samples[cursor][1])
        cursor += 1
    return cursor, due


class ReplayPlayer(QObject):
    """Real-time (with speed factor) re-emitter of a recording."""

    tick = Signal(dict)
    finished = Signal()

    def __init__(self, bus, parent=None):
        super().__init__(parent)
        self.bus = bus
        self.samples = []
        self.cursor = 0
        self.speed = 1.0
        self._elapsed0 = 0.0
        self._timer = QTimer(self)
        self._timer.setInterval(40)
        self._timer.timeout.connect(self._pump)

    def load(self, samples: list):
        self.stop()
        self.samples = list(samples)
        self.cursor = 0

    @property
    def duration(self) -> float:
        return self.samples[-1][0] if self.samples else 0.0

    @property
    def playing(self) -> bool:
        return self._timer.isActive()

    def play(self, speed: float = 1.0):
        if not self.samples:
            return
        self.speed = max(0.1, float(speed))
        self._elapsed0 = time.monotonic()
        self._timer.start()

    def pause(self):
        self._timer.stop()

    def resume(self):
        if self.samples and not self._timer.isActive():
            self._elapsed0 = time.monotonic()
            self._timer.start()

    def stop(self):
        self._timer.stop()
        self.cursor = 0

    def _pump(self):
        elapsed = time.monotonic() - self._elapsed0
        self.cursor, due = samples_due(self.samples, self.cursor, elapsed, self.speed)
        for data in due:
            self.tick.emit(data)
            self.bus.telemetry.emit(data)
        if self.cursor >= len(self.samples):
            self._timer.stop()
            self.finished.emit()
