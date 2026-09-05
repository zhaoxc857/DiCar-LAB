"""Firmware version probe: ask the car for its firmware version via
CMD fw_version; the reference firmware answers with a NOTE event
{"type":"NOTE","data":"fw_version=<version>"} plus a plain ACK.

Old firmware that does not know the command simply rejects the ACK and
stays silent - the probe then reports "未上报".
"""

from __future__ import annotations

import time

from PySide6.QtCore import QObject, QTimer, Signal


class FwVersionProbe(QObject):
    version = Signal(str)

    def __init__(self, bus, transport, parent=None):
        super().__init__(parent)
        self.bus = bus
        self.transport = transport
        self.last_version = ""
        self._deadline = 0.0
        self._waiting = False
        bus.event.connect(self._event)
        bus.connection.connect(self._on_connection)
        self._timer = QTimer(self)
        self._timer.setInterval(100)
        self._timer.timeout.connect(self._tick)

    def probe(self, timeout_s: float = 1.5):
        if self.transport is None or not self.transport.connected:
            return
        self._waiting = True
        self._deadline = time.monotonic() + timeout_s
        try:
            self.transport.command("fw_version", "")
        except Exception:
            self._waiting = False
            return
        self._timer.start()

    def _event(self, typ, obj):
        if typ != "NOTE" or not self._waiting:
            return
        data = str((obj or {}).get("data", ""))
        if data.startswith("fw_version="):
            self.last_version = data.split("=", 1)[1].strip()
            self._waiting = False
            self._timer.stop()
            self.version.emit(self.last_version)

    def _tick(self):
        if self._waiting and time.monotonic() >= self._deadline:
            self._waiting = False
            self._timer.stop()

    def _on_connection(self, ok, _text):
        if ok:
            QTimer.singleShot(800, self.probe)
