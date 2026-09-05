import os
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "CAR_LAB"


class ApplicationReleaseTests(unittest.TestCase):
    def test_python_metadata_matches_release_file(self):
        sys.path.insert(0, str(APP))
        from core.version import DISPLAY_VERSION

        self.assertEqual("DiCAR LAB v1.12.0", DISPLAY_VERSION)
        self.assertEqual(
            DISPLAY_VERSION,
            (ROOT / "VERSION.txt").read_text(encoding="utf-8").strip(),
        )

    def test_offscreen_smoke_mode_constructs_and_exits(self):
        env = os.environ.copy()
        env.update(QT_QPA_PLATFORM="offscreen", DICAR_SMOKE_TEST="1")
        result = subprocess.run(
            [sys.executable, "main.py"],
            cwd=APP,
            env=env,
            capture_output=True,
            text=True,
            timeout=90,  # 页面数量增长后，离屏构建在 CI 慢机上需要更长时间
        )
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
