import csv
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core.wave_store import load_wave_csv, save_wave_csv


class WaveStoreTests(unittest.TestCase):
    def test_round_trip_preserves_times_and_channels(self):
        times = [0.0, 0.25, 0.5]
        channels = {
            "actual_rpm": [10.0, 12.5, 20.0],
            "battery": [12.4, 12.4, 12.3],
        }
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "wave.csv"
            save_wave_csv(path, times, channels)
            loaded_times, loaded = load_wave_csv(path)
        self.assertEqual(times, loaded_times)
        self.assertEqual(channels, loaded)

    def test_missing_samples_export_as_blank_and_load_as_none(self):
        times = [0.0, 1.0, 2.0]
        channels = {"a": [1.0, None, 3.0]}
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "wave.csv"
            save_wave_csv(path, times, channels)
            with open(path, encoding="utf-8") as f:
                rows = list(csv.reader(f))
            self.assertEqual(["time", "a"], rows[0])
            self.assertEqual(["0.0", "1.0"], rows[1][:2])
            self.assertEqual("", rows[2][1])
            loaded_times, loaded = load_wave_csv(path)
        self.assertEqual(times, loaded_times)
        self.assertEqual([1.0, None, 3.0], loaded["a"])

    def test_single_channel_recording_is_replayable(self):
        times = [float(i) for i in range(5)]
        channels = {"gyro_z": [0.1 * i for i in range(5)]}
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "wave.csv"
            save_wave_csv(path, times, channels)
            loaded_times, loaded = load_wave_csv(path)
        self.assertEqual(5, len(loaded_times))
        self.assertEqual(["gyro_z"], list(loaded))


if __name__ == "__main__":
    unittest.main()
