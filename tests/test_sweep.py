import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core.sweep import parse_candidates, pick_best


class SweepTests(unittest.TestCase):
    def test_parse_candidates_sorts_and_tolerates_chinese_comma(self):
        self.assertEqual([0.6, 0.8, 1.0, 1.2], parse_candidates("1.0, 0.6，0.8 1.2"))
        self.assertEqual([], parse_candidates(""))
        with self.assertRaises(ValueError):
            parse_candidates("0.6,abc")

    def test_pick_best_uses_lowest_score(self):
        results = [{"value": 1.0, "score": 12.0}, {"value": 0.8, "score": 7.5}]
        self.assertEqual(0.8, pick_best(results)["value"])
        self.assertIsNone(pick_best([]))
        self.assertIsNone(pick_best([{"value": 1.0}]))


if __name__ == "__main__":
    unittest.main()
