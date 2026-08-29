from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]


class BrandingTests(unittest.TestCase):
    def test_dicar_launcher_is_the_only_root_launcher(self):
        self.assertTrue((ROOT / "DiCAR_Launcher.py").is_file())
        self.assertTrue((ROOT / "DiCAR_Launcher.bat").is_file())
        self.assertFalse((ROOT / "IKUN_Launcher.py").exists())
        self.assertFalse((ROOT / "IKUN_Launcher.bat").exists())

    def test_user_facing_entry_points_use_dicar_brand(self):
        files = [
            ROOT / "DiCAR_Launcher.py",
            ROOT / "DiCAR_Launcher.bat",
            ROOT / "build_launcher_windows.bat",
            ROOT / "CAR_LAB" / "main.py",
            ROOT / "CAR_LAB" / "ui" / "main_window.py",
            ROOT / "CAR_LAB" / "ui" / "theme.py",
        ]
        for path in files:
            self.assertTrue(path.is_file(), path.name)
            text = path.read_text(encoding="utf-8")
            self.assertNotIn("IKUN", text, path.name)
        self.assertIn("DiCAR LAB", (ROOT / "DiCAR_Launcher.py").read_text(encoding="utf-8"))

    def test_launcher_supports_msys_venv_layout(self):
        launcher = ROOT / "DiCAR_Launcher.py"
        self.assertTrue(launcher.is_file(), launcher.name)
        self.assertIn('VENV / "bin/python.exe"', launcher.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
