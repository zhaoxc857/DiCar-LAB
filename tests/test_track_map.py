import math
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core.track_map import TrackMapIntegrator, reconstruct_path


class TrackMapTests(unittest.TestCase):
    def test_straight_line_forward(self):
        samples = [{"t": i * 0.1, "speed": 10.0, "gyro_z": 0.0} for i in range(11)]
        points = reconstruct_path(samples, meters_per_unit=0.1)
        # 10 单位/s × 0.1 m/单位 × 1s = 1m，沿 +y 前进（航向 0°）
        x, y = points[-1]
        self.assertAlmostEqual(0.0, x, places=6)
        self.assertAlmostEqual(1.0, y, places=6)

    def test_constant_yaw_rate_draws_circle(self):
        # 90°/s、速度 1 m/s、1m/s 单位速度 → 半径 1/(2π·90/360)≈0.637m 的圆
        dt = 0.01
        samples = [{"t": i * dt, "speed": 1.0, "gyro_z": 90.0} for i in range(int(4.0 / dt))]
        points = reconstruct_path(samples, meters_per_unit=1.0)
        end_x, end_y = points[-1]
        self.assertAlmostEqual(0.0, end_x, places=1)
        self.assertAlmostEqual(0.0, end_y, places=1)  # 4s 转一圈回到原点

    def test_gap_in_time_does_not_explode(self):
        integrator = TrackMapIntegrator(meters_per_unit=1.0)
        integrator.update({"speed": 10.0, "gyro_z": 0.0}, 0.0)
        x, y = integrator.update({"speed": 10.0, "gyro_z": 0.0}, 30.0)  # 30s 断流
        self.assertEqual((0.0, 0.0), (round(x, 6), round(y, 6)),
                         "超过 0.5s 的采样间隔不应产生位置跳变")

    def test_missing_fields_are_tolerated(self):
        points = reconstruct_path([{"battery": 12.0}, {"speed": 1.0}, {}])
        self.assertEqual(3, len(points))


if __name__ == "__main__":
    unittest.main()
