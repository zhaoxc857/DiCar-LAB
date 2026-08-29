import os
import sys
import unittest
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from PySide6.QtCore import QCoreApplication, QEvent
from PySide6.QtWidgets import QApplication

from core.bus import DataBus
from ui.main_window import MainWindow
from ui.protocol_monitor import ProtocolMonitor


class DummyTransport:
    def send_obj(self, _obj):
        pass


class ProtocolMonitorLifecycleTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def test_dispose_disconnects_bus_callbacks_before_widget_deletion(self):
        bus = DataBus()
        monitor = ProtocolMonitor(bus, DummyTransport())
        self.assertTrue(hasattr(monitor, "dispose"))
        monitor.dispose()
        monitor.deleteLater()
        QCoreApplication.sendPostedEvents(None, QEvent.Type.DeferredDelete)
        self.app.processEvents()
        bus.tx_text.emit("after-delete")
        bus.rx_text.emit("after-delete")

    def test_main_window_disposes_pages_before_delete_later(self):
        source = Path(MainWindow.__module__.replace(".", "/") + ".py")
        source = ROOT / "CAR_LAB" / source
        text = source.read_text(encoding="utf-8")
        self.assertLess(text.index("dispose()"), text.index("w.deleteLater()"))


if __name__ == "__main__":
    unittest.main()
