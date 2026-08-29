from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]


class FirmwareBandwidthTests(unittest.TestCase):
    def test_core_telemetry_leaves_ack_headroom_at_9600_baud(self):
        source = (
            ROOT / "firmware" / "stm32f103_line_car" / "dctp_port.c"
        ).read_text(encoding="utf-8")
        self.assertIn("#define CORE_TELEMETRY_PERIOD_MS 250u", source)


if __name__ == "__main__":
    unittest.main()
