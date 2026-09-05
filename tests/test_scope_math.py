import os
import sys
import unittest
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core.scope_math import derive_channels


class ScopeMathTests(unittest.TestCase):
    CFG = {"speed_lab": {"error_key": "speed_error"}}

    def test_error_squared_derived(self):
        out = derive_channels({"speed_error": -3.0, "battery": 12.0}, self.CFG)
        self.assertAlmostEqual(9.0, out["@err_sq"])

    def test_power_sums_all_current_channels(self):
        out = derive_channels(
            {"battery": 11.0, "left_current": 1.5, "right_current": 2.5}, self.CFG)
        self.assertAlmostEqual(44.0, out["@power_w"])

    def test_mecanum_currents_are_included(self):
        out = derive_channels(
            {"battery": 10.0, "fl_current": 1.0, "fr_current": 1.0,
             "rl_current": 0.5, "rr_current": 0.5}, self.CFG)
        self.assertAlmostEqual(30.0, out["@power_w"])

    def test_missing_sources_yield_nothing(self):
        self.assertEqual({}, derive_channels({"battery": 12.0}, self.CFG))
        self.assertEqual({}, derive_channels({"left_current": 1.0}, self.CFG))
        self.assertEqual({}, derive_channels({"speed_error": "bad"}, self.CFG))


if __name__ == "__main__":
    unittest.main()
