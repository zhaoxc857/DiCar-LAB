import os
import sys
import tempfile
import unittest
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from PySide6.QtWidgets import QApplication

from core.bus import DataBus
from core.wave_store import load_wave_csv
from ui.scope import ScopePage


class ScopeWaveTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def _make_page(self):
        return ScopePage(DataBus(), {"vehicle": {"display_name": "t"}})

    def test_record_export_round_trip(self):
        page = self._make_page()
        page.toggle_record()
        page._tel({"actual_rpm": 10.0, "battery": 12.4})
        page._tel({"actual_rpm": 20.0})
        self.assertEqual(2, len(page.record_t))
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "wave.csv"
            page._export_wave(str(path))
            times, channels = load_wave_csv(path)
        self.assertEqual(2, len(times))
        self.assertEqual([10.0, 20.0], channels["actual_rpm"])
        self.assertEqual([12.4, None], channels["battery"])
        self.assertFalse(page.recording)

    def test_replay_mode_ignores_live_and_draws_loaded_data(self):
        page = self._make_page()
        page._tel({"actual_rpm": 1.0})
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "wave.csv"
            from core.wave_store import save_wave_csv

            save_wave_csv(path, [0.0, 1.0, 2.0], {"actual_rpm": [5.0, 6.0, 7.0]})
            page._load_wave(str(path))
        self.assertTrue(page.replay_mode)
        ts, ys = page._times_values("actual_rpm")
        self.assertEqual([0.0, 1.0, 2.0], ts)
        self.assertEqual([5.0, 6.0, 7.0], ys)
        live_count = len(page.t)
        page._tel({"actual_rpm": 999.0})
        self.assertEqual(live_count, len(page.t))
        page.exit_replay()
        self.assertFalse(page.replay_mode)
        self.assertEqual(0, len(page.t))


if __name__ == "__main__":
    unittest.main()
