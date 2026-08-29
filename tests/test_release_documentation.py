from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]


class ReleaseDocumentationTests(unittest.TestCase):
    def test_readme_names_download_and_documents_wireless_flashing(self):
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn("DiCAR-LAB-v1.8.0-Windows-x64.zip", readme)
        self.assertIn("https://github.com/zhaoxc857/DiCar_Tune/releases", readme)
        self.assertIn("无限烧录路线图", readme)
        self.assertIn("stm32flash", readme)
        self.assertIn("BOOT0", readme)

    def test_changelog_starts_with_v180_release(self):
        changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        self.assertLess(changelog.index("## v1.8.0"), changelog.index("## v1.7.0"))


if __name__ == "__main__":
    unittest.main()
