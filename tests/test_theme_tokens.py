import os
import sys
import unittest
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from ui import theme


class ThemeTokenTests(unittest.TestCase):
    def test_token_parity_between_themes(self):
        dark, light = theme.TOKENS["黑色"], theme.TOKENS["白色"]
        self.assertEqual(sorted(dark.keys()), sorted(light.keys()))
        for key in ("bg", "surface", "surface2", "border", "text", "muted",
                    "accent", "accent_hover", "accent_pressed", "accent_text",
                    "success", "warn", "danger", "scrim", "plot_bg", "plot_fg"):
            self.assertIn(key, dark)
            self.assertNotEqual(dark[key], "", key)

    def test_curve_palette_has_eight_theme_colors(self):
        for t in theme.TOKENS.values():
            self.assertEqual(8, len(t["curves"]))
            for color in t["curves"]:
                self.assertRegex(color, r"^#[0-9a-f]{6}$")

    def test_styles_are_generated_from_tokens_and_differ(self):
        self.assertTrue(theme.DARK_STYLE.startswith("QWidget"))
        self.assertTrue(theme.LIGHT_STYLE.startswith("QWidget"))
        self.assertNotEqual(theme.DARK_STYLE, theme.LIGHT_STYLE)
        # 基础模式（无壁纸）不得混入 rgba 半透明面
        self.assertNotIn("rgba(", theme.DARK_STYLE)
        self.assertNotIn("rgba(", theme.LIGHT_STYLE)

    def test_qss_covers_previously_native_widgets(self):
        for selector in ("QMenu", "QComboBox QAbstractItemView", "QCheckBox::indicator",
                         "QRadioButton::indicator", "QToolButton", "QProgressBar::chunk",
                         "QListWidget::item", "QToolTip", "QDialog",
                         "QSpinBox::up-arrow", "QScrollBar::add-line"):
            self.assertIn(selector, theme.DARK_STYLE, selector)
            self.assertIn(selector, theme.LIGHT_STYLE, selector)

    def test_dead_selectors_removed(self):
        self.assertNotIn("plotPanel", theme.DARK_STYLE)
        self.assertNotIn("scopeTime", theme.DARK_STYLE)

    def test_wallpaper_mode_turns_transparent_and_uses_rgba(self):
        qss = theme.build_qss("黑色", wallpaper=True, panel_opacity=80)
        self.assertIn("background: transparent;", qss)
        self.assertIn("rgba(", qss)
        plain = theme.build_qss("黑色")
        self.assertNotIn("rgba(", plain)

    def test_state_color_and_plot_color(self):
        theme.set_current_theme("黑色")
        self.assertEqual("#78d79a", theme.state_color("ok"))
        self.assertEqual("#e5737a", theme.state_color("bad"))
        self.assertEqual(theme.TOKENS["黑色"]["curves"][0], theme.plot_color(0))
        self.assertEqual(theme.TOKENS["黑色"]["curves"][0], theme.plot_color(8))
        self.assertEqual(theme.TOKENS["黑色"]["curves"][2], theme.plot_color(2))

    def test_no_brand_regression_in_theme_module(self):
        source = (ROOT / "CAR_LAB" / "ui" / "theme.py").read_text(encoding="utf-8")
        self.assertNotIn("IKUN", source)

    def test_stat_value_style_uses_unified_size(self):
        self.assertIn(f"QLabel#statValue {{ font-size:{theme.FONT_STAT}px", theme.LIGHT_STYLE)
        self.assertEqual(28, theme.FONT_STAT)


if __name__ == "__main__":
    unittest.main()
