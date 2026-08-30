from pathlib import Path
import unittest

import yaml


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "windows-release.yml"


class WindowsReleaseWorkflowTests(unittest.TestCase):
    def test_workflow_builds_tests_and_publishes_portable_release(self):
        self.assertTrue(WORKFLOW.is_file(), "Windows release workflow is missing")

        source = WORKFLOW.read_text(encoding="utf-8")
        workflow = yaml.load(source, Loader=yaml.BaseLoader)

        self.assertEqual(workflow["permissions"]["contents"], "write")
        self.assertIn("python -m unittest discover -s tests -v", source)
        self.assertIn("build_portable_windows.ps1", source)
        self.assertIn("build_onefile_windows.ps1", source)
        self.assertIn("actions/upload-artifact@v4", source)
        self.assertIn("softprops/action-gh-release@v2", source)


if __name__ == "__main__":
    unittest.main()
