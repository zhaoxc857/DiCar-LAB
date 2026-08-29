import os
import sys
import unittest
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from PySide6.QtWidgets import QApplication

from core.flash_job import FlashState
from ui.firmware_flash import FirmwareFlashPage
from ui.main_window import PAGE_DEFS


class FirmwareFlashPageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def test_unconfigured_page_cannot_start_a_flash(self):
        page = FirmwareFlashPage(
            {"vehicle": {"display_name": "STM32 巡线车"}}
        )
        self.assertEqual(FlashState.UNAVAILABLE, page.state.state)
        self.assertFalse(page.run_button.isEnabled())
        self.assertEqual("烧录后端尚未配置", page.reason_label.text())
        self.assertIn("STM32 巡线车", page.target_label.text())
        self.assertTrue(page.single_mode.isChecked())

    def test_tools_navigation_exposes_firmware_page_last(self):
        group, pages = PAGE_DEFS[-1]
        self.assertEqual("工具", group)
        self.assertEqual("固件烧录", pages[-1][0])
        self.assertIs(FirmwareFlashPage, pages[-1][2])


if __name__ == "__main__":
    unittest.main()
