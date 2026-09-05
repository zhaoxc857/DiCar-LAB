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


class SerialWriteErrorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def test_write_error_is_emitted_not_swallowed(self):
        from core.transport import SerialWorker

        class BrokenSerial:
            def write(self, data):
                raise OSError("port gone")

        worker = SerialWorker()
        errors = []
        worker.write_error.connect(errors.append)
        worker._serial = BrokenSerial()
        worker.write(b"SET\n")
        self.assertEqual(1, len(errors), "发送失败必须暴露给诊断，不允许静默吞掉")
        self.assertIn("port gone", errors[0])


class AckMatchingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def _make_transport(self):
        bus = DataBus()
        protocol = JsonLineProtocol(bus)
        return TransportManager(bus, protocol, {"vehicle": {"display_name": "t"}})

    def test_stale_seq_ack_with_same_key_is_dropped(self):
        import time as _time

        transport = self._make_transport()
        transport._param_inflight = {
            "kind": "SET", "key": "speed_kp", "value": 1.0,
            "seq": 7, "retry": 0, "sent_at": _time.monotonic(),
        }
        # 带 seq 但不匹配：同 key 的旧 ACK，应丢弃
        transport._handle_ack_detail(
            {"key": "speed_kp", "value": 1.0, "seq": 8, "ok": True, "error": None}
        )
        self.assertIsNotNone(transport._param_inflight)
        # 老固件不带 seq：仍按 key 兜底接受
        transport._handle_ack_detail(
            {"key": "speed_kp", "value": 1.0, "seq": None, "ok": True, "error": None}
        )
        self.assertIsNone(transport._param_inflight)
        transport.shutdown()


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
