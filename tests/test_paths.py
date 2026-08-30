import sys
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core import paths


class PathResolutionTests(unittest.TestCase):
    def test_source_run_keeps_everything_in_repository(self):
        self.assertFalse(paths.is_frozen())
        self.assertEqual(paths.resource_root(), ROOT / "CAR_LAB")
        self.assertEqual(paths.data_root(), ROOT / "CAR_LAB")

    def test_frozen_run_reads_resources_from_bundle(self):
        with mock.patch.object(sys, "frozen", True, create=True), \
             mock.patch.object(sys, "_MEIPASS", "C:/bundle", create=True):
            self.assertTrue(paths.is_frozen())
            self.assertEqual(paths.resource_root(), Path("C:/bundle"))

    def test_frozen_run_writes_user_data_outside_bundle(self):
        with mock.patch.object(sys, "frozen", True, create=True), \
             mock.patch.object(sys, "_MEIPASS", "C:/bundle", create=True), \
             mock.patch.dict("os.environ", {"LOCALAPPDATA": "C:/users/u/AppData/Local"}):
            self.assertEqual(
                paths.data_root(),
                Path("C:/users/u/AppData/Local") / paths.APP_DATA_DIRNAME,
            )

    def test_frozen_run_without_localappdata_falls_back_to_home(self):
        env = {"LOCALAPPDATA": ""}
        with mock.patch.object(sys, "frozen", True, create=True), \
             mock.patch.dict("os.environ", env, clear=False), \
             mock.patch("os.environ.get", return_value=None):
            self.assertEqual(paths.data_root(), Path.home() / paths.APP_DATA_DIRNAME)


if __name__ == "__main__":
    unittest.main()
