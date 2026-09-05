"""跨页面共享的小组件（样式全部走主题 objectName，无内联颜色）。"""
from __future__ import annotations

from PySide6.QtWidgets import QFrame, QLabel, QVBoxLayout


class ValueCard(QFrame):
    """统一数据卡：标题（muted）+ 大数字（statValue，28px，主题化）。"""

    def __init__(self, title, unit=""):
        super().__init__()
        self.setObjectName("card")
        lay = QVBoxLayout(self)
        lay.setContentsMargins(12, 8, 12, 8)
        t = QLabel(title)
        t.setObjectName("muted")
        self.val = QLabel("--")
        self.val.setObjectName("statValue")
        self.unit = unit
        lay.addWidget(t)
        lay.addWidget(self.val)

    def set_value(self, value):
        try:
            self.val.setText(f"{float(value):.2f} {self.unit}".strip())
        except Exception:
            self.val.setText(str(value))
