"""仿真闯关页：把调参练习变成关卡任务，只对内置仿真车开放（不动实车）。"""

import time
from collections import deque

from PySide6.QtCore import QTimer
from PySide6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton, QGroupBox,
    QPlainTextEdit, QMessageBox,
)
from PySide6.QtCore import QSettings

from core.missions import MISSIONS, evaluate_mission


class MissionsPage(QWidget):
    def __init__(self, bus, transport, config):
        super().__init__()
        self.bus = bus
        self.transport = transport
        self.settings = QSettings("DiCAR", "DiCAR LAB")
        self.samples = deque(maxlen=6000)
        self.mission = None
        self.phase = "idle"  # idle / settle / step / collect / done
        self.phase_until = 0.0
        self._restores = {}

        root = QVBoxLayout(self)
        root.setSpacing(10)
        intro = QLabel(
            "闯关只操作内置仿真车，实车连接时自动拒绝。每关会先注入一组『故意没调好』的参数，"
            "再发一次阶跃目标；你在仿真上改参数，软件按阶跃指标自动判分。"
        )
        intro.setObjectName("muted")
        intro.setWordWrap(True)
        root.addWidget(intro)

        for mission in MISSIONS:
            box = QGroupBox()
            row = QHBoxLayout(box)
            best = int(self.settings.value(f"missions/best/{mission['key']}", 0) or 0)
            stars = "★" * best + "☆" * (2 - best)
            title = QLabel(f"{mission['title']}  {stars}")
            title.setObjectName("panelTitle")
            brief = QLabel(mission["brief"])
            brief.setObjectName("muted")
            brief.setWordWrap(True)
            start = QPushButton("开始本关")
            start.clicked.connect(lambda _=False, m=dict(mission): self._start(m))
            left = QVBoxLayout()
            left.addWidget(title)
            left.addWidget(brief)
            row.addLayout(left, 1)
            row.addWidget(start)
            row.addWidget(QLabel("最佳：" + stars))
            root.addWidget(box)

        self.log = QPlainTextEdit()
        self.log.setReadOnly(True)
        self.log.setPlainText("选择关卡后：① 注入初始参数 → ② 自动发阶跃 → ③ 采集评判 → 结果与星级。")
        root.addWidget(self.log, 1)
        self.status = QLabel("就绪（需先在顶部连接方式选择『仿真』并连接）。")
        self.status.setObjectName("muted")
        root.addWidget(self.status)

        self.timer = QTimer(self)
        self.timer.setInterval(50)
        self.timer.timeout.connect(self._tick)
        bus.telemetry.connect(self._tel)

    # -- flow ---------------------------------------------------------------

    def _start(self, mission):
        if self.transport is None or self.transport.kind != "sim":
            QMessageBox.information(
                self, "仿真闯关", "本关卡只对『仿真』连接开放。\n请先在顶部选择仿真并点击连接。")
            return
        self.mission = mission
        self._restores = {
            key: self.transport.param_cache.get(key)
            for key in mission["param_setup"]
        }
        for key, value in mission["param_setup"].items():
            self.transport.set_param(key, value)
        self.samples.clear()
        self.phase = "settle"
        self.phase_until = time.monotonic() + 1.0
        self.timer.start()
        self.status.setText(f"{mission['title']}：初始参数已注入，1 秒后自动发阶跃…")
        self.log.appendPlainText(f"\n=== {mission['title']} ===")
        self.log.appendPlainText("注入参数：" + ", ".join(
            f"{k}={v}" for k, v in mission["param_setup"].items()))

    def _tick(self):
        if self.mission is None:
            return
        now = time.monotonic()
        if self.phase == "settle" and now >= self.phase_until:
            self.phase = "collect"
            self.phase_until = now + float(self.mission["collect_s"])
            step = self.mission["step"]
            self.transport.command(step["key"], float(step["value"]))
            self.status.setText("阶跃已发送，采集中…")
        elif self.phase == "collect" and now >= self.phase_until:
            self.phase = "done"
            self.timer.stop()
            self._finish()

    def _tel(self, data):
        if self.phase == "collect" and self.mission is not None:
            self.samples.append({
                "t": time.monotonic(),
                self.mission["target_key"]: data.get(self.mission["target_key"], 0.0),
                self.mission["sample_key"]: data.get(self.mission["sample_key"], 0.0),
            })

    def _finish(self):
        mission = self.mission
        result = evaluate_mission(mission, list(self.samples))
        # 阶跃目标归零 + 恢复关卡前的参数
        step = mission["step"]
        self.transport.command(step["key"], 0.0)
        for key, value in self._restores.items():
            if value is not None:
                self.transport.set_param(key, value)
        self.log.appendPlainText("判分：" + "；".join(result["detail"]))
        if result["passed"]:
            best = int(self.settings.value(f"missions/best/{mission['key']}", 0) or 0)
            stars = max(best, result["stars"])
            self.settings.setValue(f"missions/best/{mission['key']}", stars)
            self.log.appendPlainText(
                f"通过！{'★' * result['stars'] or '☆'}（历史最佳 {'★' * stars}）")
            self.status.setText(f"{mission['title']}：通过，获得 {'★' * result['stars'] or '☆'}")
            QMessageBox.information(
                self, mission["title"],
                f"通过！\n{'★' * result['stars'] or '☆'}\n\n"
                + "\n".join(result["detail"]))
        else:
            self.log.appendPlainText("未通过，按提示再调一轮参数后重新开始本关。")
            self.status.setText(f"{mission['title']}：未通过。")
            QMessageBox.warning(
                self, mission["title"],
                "未通过，继续加油！\n\n" + "\n".join(result["detail"]))
        self.mission = None
        self.phase = "idle"
