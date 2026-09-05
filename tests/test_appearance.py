import os
import sys
import tempfile
import unittest
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from PySide6.QtCore import QSettings
from PySide6.QtWidgets import QApplication

from core.appearance import (
    DEFAULTS,
    import_wallpaper,
    load_appearance,
    save_appearance,
    wallpaper_active,
    wallpaper_pixmap,
)


def _make_png(path: Path, color="#336699", size=64):
    from PySide6.QtGui import QColor, QPixmap

    pix = QPixmap(size, size)
    pix.fill(QColor(color))
    pix.save(str(path), "PNG")
    return path


class AppearancePersistenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.settings = QSettings(str(Path(self._tmp.name) / "app.ini"), QSettings.Format.IniFormat)

    def tearDown(self):
        self.settings.sync()
        self._tmp.cleanup()

    def test_defaults_when_empty(self):
        appearance = load_appearance(self.settings)
        self.assertEqual("", appearance["wallpaper_path"])
        self.assertEqual(DEFAULTS["blur_radius"], appearance["blur_radius"])
        self.assertEqual(DEFAULTS["panel_opacity"], appearance["panel_opacity"])

    def test_roundtrip_and_clamping(self):
        save_appearance(self.settings, {
            "wallpaper_path": "x.png", "blur_radius": 999, "panel_opacity": 5,
        })
        appearance = load_appearance(self.settings)
        self.assertEqual("x.png", appearance["wallpaper_path"])
        self.assertEqual(32, appearance["blur_radius"])   # 越界收敛到上限
        self.assertEqual(55, appearance["panel_opacity"])  # 越界收敛到下限


class WallpaperTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.png = _make_png(Path(self._tmp.name) / "wp.png")

    def tearDown(self):
        self._tmp.cleanup()

    def test_wallpaper_active_requires_existing_file(self):
        self.assertFalse(wallpaper_active({"wallpaper_path": ""}))
        self.assertFalse(wallpaper_active(
            {"wallpaper_path": os.path.join(self._tmp.name, "no.png")}))
        self.assertTrue(wallpaper_active({"wallpaper_path": str(self.png)}))

    def test_blurred_pipeline_returns_cover_sized_pixmap(self):
        for radius in (0, 8):
            pix = wallpaper_pixmap(str(self.png), radius, 200, 100)
            self.assertIsNotNone(pix)
            self.assertFalse(pix.isNull())
            self.assertEqual(200, pix.width())
            self.assertEqual(100, pix.height())
        self.assertIsNone(wallpaper_pixmap(str(Path(self._tmp.name) / "no.png"), 8, 100, 100))

    def test_import_wallpaper_copies_into_theme_dir(self):
        target_dir = Path(self._tmp.name) / "theme"
        copied = import_wallpaper(str(self.png), target_dir)
        self.assertTrue(Path(copied).is_file())
        self.assertEqual(self.png.read_bytes(), Path(copied).read_bytes())


if __name__ == "__main__":
    unittest.main()
