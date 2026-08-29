from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "CAR_LAB"
sys.path.insert(0, str(APP))

from core.config import load_vehicle_config, validate_vehicle_config


class Stm32LineCarProfileTests(unittest.TestCase):
    def load_profile(self):
        path = APP / "vehicles" / "stm32f103_line_car.yaml"
        self.assertTrue(path.is_file(), path.name)
        return load_vehicle_config(path)

    def test_uses_hc05_default_serial_speed(self):
        cfg = self.load_profile()
        self.assertEqual("stm32f103_line_car", cfg["vehicle"]["id"])
        self.assertEqual("serial", cfg["transport"]["type"])
        self.assertEqual(115200, cfg["transport"]["baudrate"])

    def test_exposes_only_required_tuning_parameters(self):
        cfg = self.load_profile()
        keys = [item["key"] for item in cfg["parameters"]]
        self.assertEqual(
            ["control_enabled", "base_pwm", "line_kp", "line_kd"],
            keys,
        )
        self.assertNotIn("power_monitor", cfg)

    def test_defines_line_following_waveform_group(self):
        cfg = self.load_profile()
        self.assertEqual(
            ["line_error", "left_cps", "right_cps", "left_pwm", "right_pwm"],
            cfg["scope_presets"]["巡线核心"],
        )
        names = cfg["channel_names"]
        for key in (
            "line_0", "line_1", "line_2", "line_3",
            "line_4", "line_5", "line_6", "line_7",
            "line_bits", "line_error", "left_count", "right_count",
            "left_cps", "right_cps", "left_pwm", "right_pwm",
        ):
            self.assertIn(key, names)

    def test_profile_has_no_validation_errors(self):
        cfg = self.load_profile()
        errors = [x for x in validate_vehicle_config(cfg) if x["severity"] == "error"]
        self.assertEqual([], errors)


if __name__ == "__main__":
    unittest.main()
