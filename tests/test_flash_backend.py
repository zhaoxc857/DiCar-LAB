import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core.flash_backend import (
    build_flash_command,
    check_firmware_size,
    classify_output_segment,
    find_stm32flash,
    firmware_size_limit,
)


class FlashBackendTests(unittest.TestCase):
    def test_find_stm32flash_locates_repo_tool(self):
        self.assertEqual("stm32flash.exe", Path(find_stm32flash(ROOT)).name)
        self.assertEqual(
            "stm32flash.exe",
            Path(find_stm32flash()).name,
            "无参调用应回退到仓库内置工具（打包后由 _MEIPASS 兜底）",
        )

    def test_find_stm32flash_returns_none_for_empty_base_without_fallback(self):
        import sys as _sys
        import unittest.mock

        with tempfile.TemporaryDirectory() as tmp:
            with unittest.mock.patch.object(_sys, "_MEIPASS", None, create=True):
                # 仓库兜底目录指向临时空目录，模拟没有工具的环境
                import core.flash_backend as fb

                real_file = fb.__file__
                try:
                    fb.__file__ = str(Path(tmp) / "core" / "flash_backend.py")
                    self.assertIsNone(fb.find_stm32flash(ROOT / "no-such-dir"))
                finally:
                    fb.__file__ = real_file

    def test_command_matches_manual_wireless_flash_recipe(self):
        cmd = build_flash_command("C:/t/stm32flash.exe", "COM6", 9600, "C:/fw/DIcai_TS.hex")
        self.assertEqual(
            [
                "C:/t/stm32flash.exe",
                "-m", "8e1",
                "-b", "9600",
                "-w", "C:/fw/DIcai_TS.hex",
                "-v",
                "-g", "0x0",
                "COM6",
            ],
            cmd,
            "PC 侧串口格式必须显式为 8E1（AN3155 偶校验），不再依赖用户猜测 HC-05 配置",
        )

    def test_command_can_disable_serial_mode_for_diagnostics(self):
        cmd = build_flash_command(
            "C:/t/stm32flash.exe", "COM6", 115200, "C:/fw/fw.bin", serial_mode="8n1"
        )
        self.assertIn("8n1", cmd)
        cmd = build_flash_command(
            "C:/t/stm32flash.exe", "COM6", 115200, "C:/fw/fw.bin", serial_mode=""
        )
        self.assertNotIn("-m", cmd)

    def test_firmware_size_limits_reject_wrong_files(self):
        self.assertIsNone(check_firmware_size("MSPM0G3507", 128 * 1024))
        message = check_firmware_size("MSPM0G3507", 128 * 1024 + 1)
        self.assertIsNotNone(message)
        self.assertIn("128KB", message)
        self.assertIsNone(check_firmware_size("STM32F1", 1024 * 1024))
        self.assertIsNotNone(check_firmware_size("STM32F1", 1024 * 1024 + 1))
        self.assertIsNone(check_firmware_size("未知系列", 10 ** 9))
        self.assertEqual(2 * 1024 * 1024, firmware_size_limit("STM32F4"))

    def test_stm32flash_progress_lines_are_parsed_not_logged(self):
        kind, percent = classify_output_segment(
            "Wrote and verified address 0x08000004 (12.34%)"
        )
        self.assertEqual("progress", kind)
        self.assertAlmostEqual(12.34, percent)
        kind, percent = classify_output_segment(
            "Wrote address 0x08000200 (100.00%) "
        )
        self.assertEqual("progress", kind)
        self.assertAlmostEqual(100.0, percent)
        for text in ("Erasing memory", "Write to memory", "Done.", "stm32flash 0.7"):
            self.assertEqual(("log", None), classify_output_segment(text))
        # 未写完的进度行（串口分块截断）不应被误判为进度
        self.assertEqual(("log", None), classify_output_segment("Wrote address 0x08000 (12"))


if __name__ == "__main__":
    unittest.main()
