"""首次运行引导：四步指路，不阻塞主窗口（show 而非 exec），可随时从主界面重开。"""

from PySide6.QtWidgets import (
    QDialog, QVBoxLayout, QHBoxLayout, QLabel, QPushButton, QStackedWidget,
    QDialogButtonBox,
)

STEPS = [
    ("欢迎来到 DiCAR LAB",
     "这是一台智能车调车工作台：实时示波器、在线 PID 调参、赛道分析、无线烧录固件。\n\n"
     "第一次使用建议先连『仿真』——不接硬件也能体验全部功能。",
     ("去连接（顶部选「仿真」后点「连接」）", 0)),
    ("读懂实时示波器",
     "示波器把 MCU 发来的每个遥测字段画成曲线：左边勾选通道，顶部可切换「速度/航向/电源」工作组，"
     "曲线区左键点两下可锁定 A/B 光标测量差值。",
     ("打开示波器页看看", 1)),
    ("在线调参与安全红线",
     "Speed/Heading Lab 里改参数会立即下发给车辆（带 ACK 回读确认）。\n\n"
     "实车调参请务必架空车辆；AI 调参的阶跃测试需要手动勾选安全确认。",
     ("去 Speed Lab", 2)),
    ("烧录固件与版本库",
     "「工具 → 固件烧录」支持 STM32F1/F4 与 TI MSPM0G3507 无线烧录；每次烧录自动存入固件版本库，"
     "写好备注就能一键回退到旧版本。",
     ("打开固件烧录页", None)),
]


class OnboardingDialog(QDialog):
    def __init__(self, goto_page=None, parent=None):
        """goto_page: callback(int) to switch main window pages; None disables jumps."""
        super().__init__(parent)
        self.goto_page = goto_page
        self.setWindowTitle("DiCAR LAB 使用引导")
        self.resize(560, 320)
        root = QVBoxLayout(self)
        self.stack = QStackedWidget()
        for title, body, (button_text, page_index) in STEPS:
            page = QLabel(f"<h3>{title}</h3><p>{body.replace(chr(10), '<br>')}</p>")
            page.setWordWrap(True)
            self.stack.addWidget(page)
        root.addWidget(self.stack, 1)
        row = QHBoxLayout()
        self.jump_btn = QPushButton("")
        self.jump_btn.clicked.connect(self._jump)
        row.addWidget(self.jump_btn)
        row.addStretch(1)
        buttons = QDialogButtonBox()
        self.back_btn = QPushButton("上一步")
        self.next_btn = QPushButton("下一步")
        self.next_btn.setObjectName("primary")
        buttons.addButton(self.back_btn, QDialogButtonBox.ActionRole)
        buttons.addButton(self.next_btn, QDialogButtonBox.ActionRole)
        self.back_btn.clicked.connect(self._back)
        self.next_btn.clicked.connect(self._next)
        row.addWidget(buttons)
        root.addLayout(row)
        self._sync()

    def _jump(self):
        index = STEPS[self.stack.currentIndex()][2][1]
        if self.goto_page is not None and index is not None:
            self.goto_page(index)

    def _back(self):
        self.stack.setCurrentIndex(max(0, self.stack.currentIndex() - 1))
        self._sync()

    def _next(self):
        if self.stack.currentIndex() >= self.stack.count() - 1:
            self.accept()
            return
        self.stack.setCurrentIndex(self.stack.currentIndex() + 1)
        self._sync()

    def _sync(self):
        index = self.stack.currentIndex()
        title, _body, (button_text, page_index) = STEPS[index]
        self.jump_btn.setText(button_text)
        self.jump_btn.setEnabled(self.goto_page is not None and page_index is not None)
        self.back_btn.setEnabled(index > 0)
        self.next_btn.setText("完成" if index == self.stack.count() - 1 else "下一步")
