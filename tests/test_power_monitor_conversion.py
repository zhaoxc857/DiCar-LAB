import os
import sys
import unittest
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from ui.power_monitor import raw_to_voltage


class RawToVoltageTests(unittest.TestCase):
    CFG = {
        "adc_bits": 12, "vref": 3.3,
        "divider_r1": 30000, "divider_r2": 10000,
        "gain": 1.0, "offset": 0.0,
    }

    def test_zero_raw_maps_to_offset(self):
        self.assertAlmostEqual(0.0, raw_to_voltage(0, self.CFG), places=6)

    def test_full_scale_uses_divider_ratio(self):
        expected = 4095 / 4096 * 3.3 * (30000 + 10000) / 10000
        self.assertAlmostEqual(expected, raw_to_voltage(4095, self.CFG), places=6)

    def test_gain_and_offset_apply(self):
        cfg = {"adc_bits": 10, "vref": 3.0, "divider_r1": 10000,
               "divider_r2": 10000, "gain": 2.0, "offset": 0.5}
        expected = 512 / 1024 * 3.0 * (10000 + 10000) / 10000 * 2.0 + 0.5
        self.assertAlmostEqual(expected, raw_to_voltage(512, cfg), places=6)

    def test_misconfiguration_returns_none(self):
        self.assertIsNone(raw_to_voltage(100, {}))
        self.assertIsNone(raw_to_voltage(100, {"adc_bits": 0, "vref": 3.3,
                                               "divider_r1": 1, "divider_r2": 1}))
        self.assertIsNone(raw_to_voltage("abc", dict(self.CFG)))


if __name__ == "__main__":
    unittest.main()
