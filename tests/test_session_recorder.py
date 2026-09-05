import json
import os
import sys
import tempfile
import unittest
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from PySide6.QtWidgets import QApplication

from core.session_recorder import SessionRecorder, load_recording, samples_due
from core.bus import DataBus


class RecorderPureTests(unittest.TestCase):
    def test_samples_due_paces_by_speed_factor(self):
        samples = [(0.1, {"a": 1}), (0.2, {"a": 2}), (0.3, {"a": 3})]
        cursor, due = samples_due(samples, 0, 0.15, speed=1.0)
        self.assertEqual(1, cursor)
        self.assertEqual([{"a": 1}], due)
        # 2x 速度下 0.15s 墙钟 = 0.3s 回放时间，三条全部到期（含边界 0.3）
        cursor, due = samples_due(samples, 0, 0.15, speed=2.0)
        self.assertEqual(3, cursor)
        self.assertEqual([{"a": 1}, {"a": 2}, {"a": 3}], due)

    def test_load_recording_skips_broken_lines(self):
        with tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False, encoding="utf-8") as handle:
            handle.write(json.dumps({"t": 0.5, "d": {"battery": 12.0}}) + "\n")
            handle.write("not-json\n")
            handle.write(json.dumps({"t": 1.5, "d": {"battery": 11.9}}) + "\n")
            path = handle.name
        try:
            samples = load_recording(path)
            self.assertEqual(2, len(samples))
            self.assertEqual(0.5, samples[0][0])
        finally:
            os.unlink(path)


class SessionRecorderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def test_record_write_stop_roundtrip(self):
        import unittest.mock

        bus = DataBus()
        recorder = SessionRecorder(bus)
        with tempfile.TemporaryDirectory() as tmp:
            with unittest.mock.patch(
                "core.session_recorder.recordings_dir",
                return_value=Path(tmp),
            ):
                path = recorder.start()
                bus.telemetry.emit({"battery": 12.0, "speed": 1.0, "name": "x"})
                bus.telemetry.emit({"battery": 11.9})
                out = recorder.stop()
            self.assertEqual(2, recorder.count)
            self.assertTrue(str(out).startswith(str(Path(tmp))))
            samples = load_recording(out)
            self.assertEqual(2, len(samples))
            self.assertEqual(12.0, samples[0][1]["battery"])


if __name__ == "__main__":
    unittest.main()
