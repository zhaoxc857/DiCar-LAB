import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core.flash_backend import build_flash_command, find_stm32flash


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
                "-b", "9600",
                "-w", "C:/fw/DIcai_TS.hex",
                "-v",
                "-g", "0x0",
                "COM6",
            ],
            cmd,
        )


if __name__ == "__main__":
    unittest.main()
