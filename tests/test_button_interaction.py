import os
import sys
import unittest
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from PySide6.QtCore import QPoint, Qt
from PySide6.QtTest import QTest
from PySide6.QtWidgets import QApplication, QLineEdit, QPushButton, QVBoxLayout, QWidget

from ui.theme import DARK_STYLE, LIGHT_STYLE


class ButtonInteractionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def render_signature(self, style, object_name="", action=None):
        host = QWidget()
        host.setStyleSheet(style)
        layout = QVBoxLayout(host)
        button = QPushButton("")
        button.setObjectName(object_name)
        button.setFixedSize(120, 44)
        focus_sink = QLineEdit()
        layout.addWidget(button)
        layout.addWidget(focus_sink)
        host.show()
        focus_sink.setFocus()
        self.app.processEvents()

        before_geometry = button.geometry()
        if action == "pressed":
            QTest.mousePress(
                button,
                Qt.MouseButton.LeftButton,
                pos=QPoint(60, 22),
            )
        elif action == "focused":
            button.setFocus(Qt.FocusReason.TabFocusReason)
        elif action == "disabled":
            button.setEnabled(False)
        self.app.processEvents()

        image = button.grab().toImage()
        signature = (
            image.pixelColor(12, 12).name(),
            image.pixelColor(1, 22).name(),
        )
        self.assertEqual(before_geometry, button.geometry())
        if action == "pressed":
            QTest.mouseRelease(
                button,
                Qt.MouseButton.LeftButton,
                pos=QPoint(60, 22),
            )
        host.close()
        return signature

    def test_every_theme_and_semantic_button_changes_when_pressed(self):
        for style in (DARK_STYLE, LIGHT_STYLE):
            for name in ("", "primary", "danger"):
                with self.subTest(style=style[:20], name=name):
                    self.assertNotEqual(
                        self.render_signature(style, name),
                        self.render_signature(style, name, "pressed"),
                    )

    def test_focus_and_disabled_states_render_distinctly(self):
        for style in (DARK_STYLE, LIGHT_STYLE):
            normal = self.render_signature(style, "primary")
            self.assertNotEqual(
                normal,
                self.render_signature(style, "primary", "focused"),
            )
            self.assertNotEqual(
                normal,
                self.render_signature(style, "primary", "disabled"),
            )


if __name__ == "__main__":
    unittest.main()
