"""中央控件画布：壁纸（cover 缩放）+ 主题色 scrim 打底，内容浮于其上。

无壁纸或图片失效时回退纯色主题底，观感与关闭壁纸完全一致。
"""
from __future__ import annotations

from PySide6.QtGui import QColor, QPainter
from PySide6.QtWidgets import QWidget

import ui.theme as theme
from core.appearance import wallpaper_pixmap


class WallpaperCanvas(QWidget):
    def __init__(self, appearance_getter, parent=None):
        super().__init__(parent)
        self._appearance_getter = appearance_getter
        self.setContentsMargins(0, 0, 0, 0)

    def paintEvent(self, event):
        t = theme.tokens(theme.current_theme)
        appearance = self._appearance_getter()
        painter = QPainter(self)
        pixmap = wallpaper_pixmap(
            appearance.get("wallpaper_path", ""),
            appearance.get("blur_radius", 0),
            self.width(), self.height(),
        )
        if pixmap is not None:
            painter.drawPixmap(0, 0, pixmap)
            scrim = QColor(t["scrim"])
            scrim.setAlpha(int(t["scrim_alpha"]))
            painter.fillRect(self.rect(), scrim)
        else:
            painter.fillRect(self.rect(), QColor(t["bg"]))
        painter.end()
