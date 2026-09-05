import os
import sys
import unittest
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core.missions import MISSIONS, evaluate_mission


def step_response(final, overshoot_pct=0.0, dt=0.02, duration_s=4.0,
                  target_key="target_rpm", sample_key="actual_rpm"):
    """Synthetic second-order-ish step response for evaluation tests."""
    samples = []
    peak = final * (1.0 + overshoot_pct / 100.0)
    n = int(duration_s / dt)
    peak_idx = int(n * 0.1)
    for i in range(n):
        t = i * dt
        if i <= peak_idx:
            value = final * (i / max(1, peak_idx))
        else:
            value = peak + (final - peak) * min(1.0, (i - peak_idx) / (n * 0.2))
        samples.append({"t": t, target_key: final, sample_key: value})
    return samples


class MissionEvaluationTests(unittest.TestCase):
    def test_clean_response_passes_overshoot_mission_with_stars(self):
        mission = MISSIONS[0]
        samples = step_response(500.0, overshoot_pct=5.0)
        result = evaluate_mission(mission, samples)
        self.assertTrue(result["passed"])
        self.assertEqual(2, result["stars"])

    def test_overshooting_response_fails(self):
        mission = MISSIONS[0]
        samples = step_response(500.0, overshoot_pct=40.0)
        result = evaluate_mission(mission, samples)
        self.assertFalse(result["passed"])
        self.assertEqual(0, result["stars"])

    def test_insufficient_samples_reported(self):
        result = evaluate_mission(MISSIONS[0], [{"t": 0.0, "target": 1, "actual": 0}])
        self.assertFalse(result["passed"])
        self.assertIn("样本不足", result["detail"])

    def test_all_missions_have_required_fields(self):
        for mission in MISSIONS:
            for field in ("key", "title", "brief", "param_setup", "step",
                          "collect_s", "sample_key", "target_key", "rules", "stars"):
                self.assertIn(field, mission, mission.get("key"))


if __name__ == "__main__":
    unittest.main()
