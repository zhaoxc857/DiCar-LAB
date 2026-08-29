from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
FW = ROOT / "firmware" / "stm32f103_line_car"


class FirmwareProtocolContractTests(unittest.TestCase):
    def read_firmware_file(self, name):
        path = FW / name
        self.assertTrue(path.is_file(), name)
        return path.read_text(encoding="utf-8")

    def test_adapter_keeps_existing_car_app_api(self):
        header = self.read_firmware_file("dctp_port.h")
        for symbol in (
            "dctp_port_init",
            "dctp_port_poll",
            "dctp_port_get_tuning",
            "dctp_port_set_enabled",
            "dctp_port_set_telemetry",
        ):
            self.assertIn(symbol, header)

    def test_adapter_uses_json_line_messages_and_interrupt_tx(self):
        source = self.read_firmware_file("dctp_port.c")
        for token in ('"GET"', '"SET"', '"CMD"'):
            self.assertIn(token, source)
        for token in (r'\"ACK\"', r'\"TEL\"'):
            self.assertIn(token, source)
        self.assertIn("HAL_UART_Transmit_IT", source)
        self.assertNotIn("HAL_UART_Transmit(", source)

    def test_adapter_exposes_exactly_four_parameter_keys(self):
        source = self.read_firmware_file("dctp_port.c")
        for key in ("control_enabled", "base_pwm", "line_kp", "line_kd"):
            self.assertIn(f'"{key}"', source)


if __name__ == "__main__":
    unittest.main()
