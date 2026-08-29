from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]


class BrandingDocumentationTests(unittest.TestCase):
    def test_current_version_and_startup_checks_use_dicar(self):
        version = (ROOT / "VERSION.txt").read_text(encoding="utf-8")
        startup = (ROOT / "CAR_LAB" / "core" / "startup_check.py").read_text(encoding="utf-8")
        self.assertTrue(version.startswith("DiCAR LAB v"))
        self.assertNotIn("IKUN", startup)

    def test_current_user_guides_use_dicar_launcher(self):
        for relative in (
            "README.md",
            "README_开发者.txt",
            "README_小白用户.txt",
            "CAR_LAB/README.md",
        ):
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertNotIn("IKUN_Launcher", text, relative)
        self.assertTrue((ROOT / "README.md").read_text(encoding="utf-8").startswith("# DiCAR LAB"))


if __name__ == "__main__":
    unittest.main()
