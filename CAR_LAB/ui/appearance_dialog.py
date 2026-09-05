"""外观设置对话框：主题切换、自定义壁纸背景、毛玻璃强度与面板不透明度。

所有改动实时预览（persist=False），「确定」才持久化，「取消」回滚到打开
时的状态。
"""
from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import Qt, QTimer
from PySide6.QtWidgets import (
    QComboBox, QDialog, QDialogButtonBox, QFileDialog, QHBoxLayout, QLabel,
    QPushButton, QSlider, QVBoxLayout, QWidget,
)

from core.appearance import (
    BLUR_MAX, DEFAULTS, OPACITY_MAX, OPACITY_MIN, import_wallpaper,
    wallpaper_active,
)
from core.paths import data_root


class AppearanceDialog(QDialog):
    def __init__(self, theme_name: str, appearance: dict,
                 on_theme, on_appearance, parent=None):
        """on_theme(name) 切主题；on_appearance(dict, persist) 应用外观。"""
        super().__init__(parent)
        self.on_theme = on_theme
        self.on_appearance = on_appearance
        self._original_theme = theme_name
        self._original = dict(appearance)
        self._dirty_timer = QTimer(self)
        self._dirty_timer.setSingleShot(True)
        self._dirty_timer.setInterval(120)  # 滑条拖动防抖，避免频繁重模糊
        self._dirty_timer.timeout.connect(self._emit)

        self.setWindowTitle("外观设置")
        self.resize(520, 360)
        root = QVBoxLayout(self)

        theme_row = QHBoxLayout()
        theme_row.addWidget(QLabel("主题"))
        self.theme_combo = QComboBox()
        self.theme_combo.addItems(["黑色", "白色"])
        self.theme_combo.setCurrentText(theme_name)
        self.theme_combo.currentTextChanged.connect(self._theme_changed)
        theme_row.addWidget(self.theme_combo)
        theme_row.addStretch(1)
        root.addLayout(theme_row)

        wp_row = QHBoxLayout()
        self.wp_label = QLabel(self._wallpaper_text())
        self.wp_label.setObjectName("muted")
        pick = QPushButton("选择背景图片…")
        pick.clicked.connect(self._pick_wallpaper)
        clear = QPushButton("清除")
        clear.clicked.connect(self._clear_wallpaper)
        wp_row.addWidget(self.wp_label, 1)
        wp_row.addWidget(pick)
        wp_row.addWidget(clear)
        root.addLayout(wp_row)
        root.addWidget(QLabel("背景图片将复制一份到应用数据目录；毛玻璃即「壁纸预模糊 + 半透明面板」。"))

        root.addWidget(QLabel(f"毛玻璃强度（模糊半径 0–{BLUR_MAX}px，0 = 关闭）"))
        self.blur_slider = QSlider(Qt.Orientation.Horizontal)
        self.blur_slider.setRange(0, BLUR_MAX)
        self.blur_slider.setValue(int(appearance.get("blur_radius", DEFAULTS["blur_radius"])))
        self.blur_slider.valueChanged.connect(self._schedule)
        root.addWidget(self.blur_slider)

        root.addWidget(QLabel(f"面板不透明度（{OPACITY_MIN}–{OPACITY_MAX}%）"))
        self.opacity_slider = QSlider(Qt.Orientation.Horizontal)
        self.opacity_slider.setRange(OPACITY_MIN, OPACITY_MAX)
        self.opacity_slider.setValue(int(appearance.get("panel_opacity", DEFAULTS["panel_opacity"])))
        self.opacity_slider.valueChanged.connect(self._schedule)
        root.addWidget(self.opacity_slider)

        self.state = QLabel("")
        self.state.setObjectName("muted")
        root.addWidget(self.state)

        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel)
        # 未加载 Qt 中文翻译时标准按钮默认显示 OK/Cancel，显式指定中文。
        buttons.button(QDialogButtonBox.StandardButton.Ok).setText("确定")
        buttons.button(QDialogButtonBox.StandardButton.Cancel).setText("取消")
        reset = QPushButton("恢复默认")
        buttons.addButton(reset, QDialogButtonBox.ActionRole)
        reset.clicked.connect(self._reset)
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        root.addWidget(buttons)

    # -- helpers ------------------------------------------------------------

    def _current(self) -> dict:
        return {
            "wallpaper_path": self._original.get("wallpaper_path", "")
            if not hasattr(self, "_picked_path") else self._picked_path,
            "blur_radius": self.blur_slider.value(),
            "panel_opacity": self.opacity_slider.value(),
        }

    def _wallpaper_text(self) -> str:
        path = self._original.get("wallpaper_path", "")
        if hasattr(self, "_picked_path"):
            path = self._picked_path
        return f"当前背景：{path}" if path else "当前背景：未设置（使用纯色主题）"

    def _theme_changed(self, name):
        self.on_theme(name)

    def _schedule(self):
        self._dirty_timer.start()

    def _emit(self):
        self.on_appearance(self._current(), False)
        active = wallpaper_active(self._current())
        self.state.setText(
            "毛玻璃已开启（实时预览中，确定后保存）" if active
            else "未设置背景图片：仅应用不透明度到纯色主题（无毛玻璃效果）")

    def _pick_wallpaper(self):
        path, _ = QFileDialog.getOpenFileName(
            self, "选择背景图片", "",
            "图片 (*.png *.jpg *.jpeg *.bmp *.webp);;所有文件 (*)")
        if not path:
            return
        try:
            self._picked_path = import_wallpaper(path, data_root() / "theme")
        except OSError as exc:
            self.state.setText(f"复制图片失败：{exc}")
            return
        self.wp_label.setText(self._wallpaper_text())
        self._schedule()

    def _clear_wallpaper(self):
        self._picked_path = ""
        self.wp_label.setText(self._wallpaper_text())
        self._schedule()

    def _reset(self):
        self._picked_path = DEFAULTS["wallpaper_path"]
        self.wp_label.setText(self._wallpaper_text())
        self.blur_slider.setValue(DEFAULTS["blur_radius"])
        self.opacity_slider.setValue(DEFAULTS["panel_opacity"])
        self._theme_changed(self._original_theme)

    def accept(self):
        self.on_appearance(self._current(), True)
        super().accept()

    def reject(self):
        # 回滚到打开时的状态
        if hasattr(self, "_picked_path"):
            del self._picked_path
        self.on_theme(self._original_theme)
        self.on_appearance(self._original, True)
        super().reject()
