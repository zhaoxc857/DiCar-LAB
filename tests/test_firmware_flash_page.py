import os
import sys
import unittest
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from PySide6.QtWidgets import QApplication

from core.flash_job import FlashJobState, FlashState
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

    def test_flash_family_defaults_to_f1_and_guidance_matches(self):
        page = FirmwareFlashPage(
            {"vehicle": {"display_name": "STM32 巡线车"}}
        )
        self.assertEqual("STM32F1", page.family_combo.currentText())
        self.assertIn("BOOT0", page.log.toPlainText())

    def test_flash_family_follows_vehicle_profile(self):
        page = FirmwareFlashPage(
            {
                "vehicle": {"display_name": "STM32F4 · 双电机智能车"},
                "flash": {"family": "STM32F4"},
            }
        )
        self.assertEqual("STM32F4", page.family_combo.currentText())
        guidance = page.log.toPlainText()
        self.assertIn("STM32F4", guidance)
        self.assertIn("USART1", guidance)
        self.assertIn("扇区擦除", guidance)

    def test_unknown_flash_family_falls_back_to_f1(self):
        page = FirmwareFlashPage(
            {
                "vehicle": {"display_name": "未知芯片车"},
                "flash": {"family": "STM32H7"},
            }
        )
        self.assertEqual("STM32F1", page.family_combo.currentText())

    def test_mspm0_family_locks_baud_and_enables_builtin_backend(self):
        page = FirmwareFlashPage(
            {
                "vehicle": {"display_name": "TI MSPM0 双电机智能车"},
                "flash": {"family": "MSPM0G3507"},
            }
        )
        self.assertEqual("MSPM0G3507", page.family_combo.currentText())
        self.assertEqual("9600", page.baud_combo.currentText())
        self.assertFalse(page.baud_combo.isEnabled())
        self.assertIn("未实板验证", page.log.toPlainText())
        self.assertIn("PA10", page.log.toPlainText())
        # The built-in BSL driver needs no external tool.
        self.assertEqual("就绪", page.reason_label.text())
        self.assertTrue(page.run_button.isEnabled())

    def test_switching_back_to_stm32_without_backend_disables_run(self):
        page = FirmwareFlashPage(
            {
                "vehicle": {"display_name": "TI MSPM0 双电机智能车"},
                "flash": {"family": "MSPM0G3507"},
            }
        )
        page.family_combo.setCurrentText("STM32F1")
        self.assertEqual("烧录后端尚未配置", page.reason_label.text())
        self.assertFalse(page.run_button.isEnabled())

    def test_mspm0_rejects_oversize_firmware_before_touching_serial(self):
        page = FirmwareFlashPage(
            {
                "vehicle": {"display_name": "TI MSPM0 双电机智能车"},
                "flash": {"family": "MSPM0G3507"},
            }
        )
        import tempfile

        with tempfile.NamedTemporaryFile(suffix=".bin", delete=False) as handle:
            handle.write(b"\xff" * (128 * 1024 + 1))
            oversized = handle.name
        page.firmware_path.setText(oversized)
        page.port_edit.setText("COM9")
        page._start_flash()
        self.assertIn("128KB", page.reason_label.text())
        self.assertEqual(FlashState.IDLE, page.state.state)
        self.assertIsNone(page.worker)


if __name__ == "__main__":
    unittest.main()


class FirmwareFlashBackendTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def _make_page(self, backend=None):
        from ui.firmware_flash import FirmwareFlashPage

        return FirmwareFlashPage(
            {"vehicle": {"display_name": "STM32 巡线车"}},
            transport=None,
            flash_backend=backend,
        )

    def test_backend_present_enables_flashing(self):
        from core.flash_job import FlashState

        page = self._make_page(backend="C:/t/stm32flash.exe")
        self.assertEqual(FlashState.IDLE, page.state.state)
        self.assertTrue(page.run_button.isEnabled())
        self.assertFalse(page.continuous_mode.isEnabled())

    def test_validation_rejects_missing_firmware(self):
        from core.flash_job import FlashState

        page = self._make_page(backend="C:/t/stm32flash.exe")
        page.firmware_path.setText("Z:/no/such/firmware.hex")
        page.port_edit.setText("COM6")
        page._start_flash()
        self.assertIn("固件", page.reason_label.text())
        self.assertEqual(FlashState.IDLE, page.state.state)
        self.assertTrue(page.run_button.isEnabled())

    def test_validation_rejects_empty_port(self):
        from core.flash_job import FlashState

        page = self._make_page(backend="C:/t/stm32flash.exe")
        page.firmware_path.setText("C:/fw/DIcai_TS.hex")
        page.port_edit.setText("")
        page._start_flash()
        self.assertIn("端口", page.reason_label.text())
        self.assertEqual(FlashState.IDLE, page.state.state)


class FirmwareProgressAndCancelTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        import tempfile

        from core.firmware_store import FirmwareStore

        self._tmp = tempfile.TemporaryDirectory()
        base = Path(self._tmp.name)
        self.store = FirmwareStore(base / "lib.db", base / "library")

    def tearDown(self):
        self._tmp.cleanup()

    def _make_page(self, config, backend=None):
        return FirmwareFlashPage(
            config, transport=None, flash_backend=backend,
            firmware_store=self.store,
        )

    def _make_mspm0_page(self):
        return self._make_page(
            {
                "vehicle": {"id": "vtest", "display_name": "TI MSPM0 双电机智能车"},
                "flash": {"family": "MSPM0G3507"},
            }
        )

    def test_mspm0_stages_and_progress_drive_visual_bar(self):
        page = self._make_mspm0_page()
        page.state = FlashJobState(FlashState.FLASHING)
        page._on_mspm0_stage("erasing")
        self.assertEqual("擦除主闪存…", page.stage_label.text())
        self.assertEqual(0, page.progress_bar.maximum(), "擦除阶段应为忙碌指示")
        page._on_mspm0_progress(256, 1024)
        self.assertEqual(25, page.progress_bar.value())
        self.assertIn("25%", page.stage_label.text())
        page._on_mspm0_stage("verifying")
        self.assertEqual("回读校验…", page.stage_label.text())

    def test_cancelled_worker_lands_on_cancelled_not_failed(self):
        page = self._make_mspm0_page()
        page.state = FlashJobState(FlashState.FLASHING)

        class FakeWorker:
            was_cancelled = True

        page.worker = FakeWorker()
        page._on_mspm0_finished(1)
        self.assertEqual(FlashState.IDLE, page.state.state)
        self.assertEqual("已取消", page.reason_label.text())

    def test_failed_worker_still_reports_failure(self):
        page = self._make_mspm0_page()
        page.state = FlashJobState(FlashState.FLASHING)

        class FakeWorker:
            was_cancelled = False

        page.worker = FakeWorker()
        page._on_mspm0_finished(1)
        self.assertEqual(FlashState.IDLE, page.state.state)
        self.assertIn("烧录失败", page.reason_label.text())

    def test_stm32_progress_segments_drive_bar_without_log_spam(self):
        page = self._make_page(
            {"vehicle": {"display_name": "STM32 巡线车"}},
            backend="C:/t/stm32flash.exe",
        )
        page.state = FlashJobState(FlashState.FLASHING)
        page._handle_stm_segment("stm32flash 0.7")
        page._handle_stm_segment("Erasing memory")
        self.assertEqual(0, page.progress_bar.maximum())
        page._handle_stm_segment("Wrote and verified address 0x08000100 (42.5%)")
        self.assertAlmostEqual(42.5, page.progress_bar.value(), delta=1)
        plain = page.log.toPlainText()
        self.assertNotIn("Wrote", plain, "写入进度不得刷屏日志区")
        self.assertIn("Erasing memory", plain)

    def test_serial_mode_combo_defaults_to_even_parity(self):
        page = self._make_page(
            {"vehicle": {"display_name": "STM32 巡线车"}},
            backend="C:/t/stm32flash.exe",
        )
        self.assertEqual("8e1", page._serial_mode())
        self.assertTrue(page.mode_combo.isEnabled())

    def test_mspm0_disables_serial_mode_selector(self):
        page = self._make_mspm0_page()
        self.assertFalse(page.mode_combo.isEnabled(), "TI ROM BSL 固定 9600-8N1")

    def test_flash_record_writes_version_library(self):
        page = self._make_mspm0_page()
        firmware = Path(self._tmp.name) / "fw.bin"
        firmware.write_bytes(b"\x01" * 32)
        page._record_version("MSPM0G3507", str(firmware))
        self.assertIsNotNone(page._active_version_id)
        rows = self.store.list(vehicle="vtest")
        self.assertEqual(1, len(rows))
        self.assertEqual("MSPM0G3507", rows[0]["family"])
        self.assertIn("@", rows[0]["note"], "空备注自动用 文件名 @ 时间")
        page._finish_version_record("ok")
        self.assertEqual("ok", self.store.get(rows[0]["id"])["result"])
        self.assertIsNone(page._active_version_id)
