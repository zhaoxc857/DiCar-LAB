from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "CAR_LAB"
sys.path.insert(0, str(APP))

from core.config import load_vehicle_config, validate_vehicle_config


class Stm32F4ProfileTests(unittest.TestCase):
    def load_profile(self):
        path = APP / "vehicles" / "stm32f4_dual_motor.yaml"
        self.assertTrue(path.is_file(), path.name)
        return load_vehicle_config(path)

    def test_declares_f4_flash_family_and_serial_transport(self):
        cfg = self.load_profile()
        self.assertEqual("stm32f4_dual_motor", cfg["vehicle"]["id"])
        self.assertEqual("STM32F4", cfg["flash"]["family"])
        self.assertEqual("serial", cfg["transport"]["type"])
        self.assertEqual(115200, cfg["transport"]["baudrate"])

    def test_profile_has_no_validation_errors(self):
        issues = validate_vehicle_config(self.load_profile())
        errors = [i for i in issues if i["severity"] == "error"]
        self.assertEqual([], errors)

    def test_f103_profile_also_declares_a_flash_family(self):
        cfg = load_vehicle_config(APP / "vehicles" / "stm32f103_line_car.yaml")
        self.assertEqual("STM32F1", cfg["flash"]["family"])


if __name__ == "__main__":
    unittest.main()
