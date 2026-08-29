import os
import sys
import time
import unittest
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from PySide6.QtWidgets import QApplication

from core.bus import DataBus
from core.protocol import JsonLineProtocol
from core.transport import TransportManager


class SerialThreadingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def _make_transport(self):
        bus = DataBus()
        protocol = JsonLineProtocol(bus)
        return TransportManager(bus, protocol, {"vehicle": {"display_name": "t"}})

    def _spin(self, seconds=3.0):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            self.app.processEvents()
            time.sleep(0.02)

    def test_connect_serial_returns_immediately_without_blocking(self):
        transport = self._make_transport()
        messages = []
        transport.bus.connection.connect(lambda ok, text: messages.append(text))
        transport.connect_serial("COM_DOES_NOT_EXIST_99", 9600)
        # 异步打开：调用立即返回，未同步置为已连接
        self.assertFalse(transport.connected)
        self.assertEqual("serial", transport.kind)
        self.assertTrue(any("连接中" in m for m in messages))
        self._spin()
        self.assertIsNone(transport.kind)
        self.assertTrue(any("连接失败" in m for m in messages))
        transport.shutdown()

    def test_shutdown_stops_worker_thread(self):
        transport = self._make_transport()
        transport.connect_serial("COM_DOES_NOT_EXIST_99", 9600)
        self._spin()
        transport.shutdown()
        self.assertFalse(transport._serial_thread.isRunning())


if __name__ == "__main__":
    unittest.main()


class FakeSerial:
    """Minimal pyserial stand-in recording close() calls."""

    last_instance = None

    def __init__(self, port, baud, timeout=None, write_timeout=None):
        self.port = port
        self.in_waiting = 0
        self.close_called = False
        FakeSerial.last_instance = self

    def read(self, n):
        return b""

    def write(self, data):
        pass

    def close(self):
        self.close_called = True


class SerialCloseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def _spin(self, seconds=3.0):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            self.app.processEvents()
            time.sleep(0.02)

    def test_disconnect_closes_an_active_serial_port(self):
        import unittest.mock

        import core.transport as transport_module

        transport = TransportManager(
            DataBus(), JsonLineProtocol(DataBus()), {"vehicle": {"display_name": "t"}}
        )
        with unittest.mock.patch.object(
            transport_module.serial, "Serial", FakeSerial
        ):
            transport.connect_serial("COMFAKE", 9600)
            self._spin()
            self.assertTrue(transport.connected)
            fake = FakeSerial.last_instance
            self.assertIsNotNone(fake)
            self.assertFalse(fake.close_called)
            transport.disconnect()
            self._spin()
            self.assertTrue(fake.close_called)
        transport.shutdown()
