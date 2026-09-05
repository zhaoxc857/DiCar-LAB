"""整车下线检查页：把自动遥测检查与人工确认项串成交付检查单，出 HTML 报告。"""

import time
from collections import deque

from PySide6.QtCore import Qt, QTimer, QUrl
from PySide6.QtGui import QDesktopServices
from PySide6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton, QTreeWidget,
    QTreeWidgetItem, QGroupBox, QMessageBox, QLineEdit, QCheckBox,
)

from core.fw_version import FwVersionProbe
from core.qc_report import build_qc_report_html
from core.paths import data_root


class QcChecklistPage(QWidget):
    def __init__(self, bus, transport, config):
        super().__init__()
        self.bus = bus
        self.transport = transport
        self.config = config
        self.probe = FwVersionProbe(bus, transport, self)
        self.probe.version.connect(self._on_fw_version)
        self.fw_version = ""
        self.running = False
        self.started_at = 0.0
        self.last_tel = None
        self.tel_intervals = deque(maxlen=200)
        self.gyro_samples = []
        self.battery_min = None

        power_cfg = config.get("power_monitor", {}) or {}
        self.warn_voltage = float(power_cfg.get("warning_voltage", 10.8))
        self.speed_cfg = config.get("track_analysis", {}) or {}

        root = QVBoxLayout(self)
        root.setSpacing(10)

        bar = QHBoxLayout()
        bar.addWidget(QLabel("检查员"))
        self.operator = QLineEdit()
        self.operator.setPlaceholderText("姓名/工号（写进报告）")
        bar.addWidget(self.operator, 1)
        self.run_btn = QPushButton("开始检查")
        self.run_btn.setObjectName("primary")
        self.run_btn.clicked.connect(self._start)
        bar.addWidget(self.run_btn)
        self.report_btn = QPushButton("生成报告")
        self.report_btn.clicked.connect(self._report)
        bar.addWidget(self.report_btn)
        root.addLayout(bar)

        self.tree = QTreeWidget()
        self.tree.setHeaderLabels(["检查项", "方式", "结果", "数据 / 说明"])
        self.tree.setColumnWidth(0, 220)
        self.tree.setColumnWidth(1, 60)
        self.tree.setAlternatingRowColors(True)
        root.addWidget(self.tree, 1)

        manual_box = QGroupBox("人工确认项（勾选 = 已现场确认）")
        manual_layout = QVBoxLayout(manual_box)
        self.manual_checks = {}
        for key, text in (
            ("motor_dir", "左右电机方向正确（前进对应正转）"),
            ("estop", "物理急停/断电开关有效"),
            ("mechanical", "机械紧固、走线、传感器安装检查完毕"),
        ):
            check = QCheckBox(text)
            self.manual_checks[key] = check
            manual_layout.addWidget(check)
        root.addWidget(manual_box)

        self.status = QLabel("连接车辆后点击「开始检查」；自动项约需 8 秒。")
        self.status.setObjectName("muted")
        self.status.setWordWrap(True)
        root.addWidget(self.status)

        self._items = [
            {"key": "comm", "name": "通信质量（TEL ≥ 20Hz 持续 5s）", "kind": "auto", "state": "pending", "detail": ""},
            {"key": "battery", "name": f"电池电压（≥ {self.warn_voltage}V）", "kind": "auto", "state": "pending", "detail": ""},
            {"key": "imu", "name": "IMU 静止零偏（|gyro_z| 均值 < 2°/s）", "kind": "auto", "state": "pending", "detail": ""},
            {"key": "fw", "name": "固件版本可读取（CMD fw_version）", "kind": "auto", "state": "pending", "detail": ""},
            {"key": "motor_dir", "name": "电机方向", "kind": "manual", "state": "pending", "detail": ""},
            {"key": "estop", "name": "物理急停", "kind": "manual", "state": "pending", "detail": ""},
            {"key": "mechanical", "name": "机械与走线", "kind": "manual", "state": "pending", "detail": ""},
        ]
        self._render_items()
        bus.telemetry.connect(self._tel)
        self._timer = QTimer(self)
        self._timer.setInterval(200)
        self._timer.timeout.connect(self._tick)
        self._timer.start()

    # -- rendering ----------------------------------------------------------

    def _find(self, key):
        for item in self._items:
            if item["key"] == key:
                return item
        return None

    def _render_items(self):
        self.tree.clear()
        state_text = {"pass": "✓ 通过", "fail": "✗ 未通过", "pending": "… 待检", "running": "… 检查中"}
        for item in self._items:
            entry = QTreeWidgetItem([
                item["name"],
                "自动" if item["kind"] == "auto" else "人工",
                state_text.get(item["state"], item["state"]),
                item["detail"],
            ])
            self.tree.addTopLevelItem(entry)

    def _set(self, key, state, detail=""):
        item = self._find(key)
        if item is None:
            return
        item["state"] = state
        if detail:
            item["detail"] = detail
        self._render_items()

    # -- auto checks ---------------------------------------------------------

    def _start(self):
        if self.transport is None or not self.transport.connected:
            QMessageBox.information(self, "下线检查", "请先连接车辆再开始检查。")
            return
        self.running = True
        self.started_at = time.monotonic()
        self.tel_intervals.clear()
        self.gyro_samples = []
        self.battery_min = None
        self.last_tel = None
        for item in self._items:
            if item["kind"] == "auto":
                item["state"] = "running" if item["key"] != "fw" else "pending"
                item["detail"] = ""
            else:
                item["state"] = "pending"
                item["detail"] = ""
        self._render_items()
        self.probe.probe()
        self.status.setText("检查中：请让车辆保持静止、架空（不要上跑道）。")

    def _tel(self, data):
        if not self.running:
            return
        now = time.monotonic()
        if self.last_tel is not None:
            self.tel_intervals.append(now - self.last_tel)
        self.last_tel = now
        battery = data.get("battery")
        if isinstance(battery, (int, float)):
            self.battery_min = battery if self.battery_min is None else min(self.battery_min, battery)
        gyro = data.get("gyro_z")
        if isinstance(gyro, (int, float)) and now - self.started_at <= 4.0:
            self.gyro_samples.append(float(gyro))

    def _tick(self):
        if not self.running:
            # 人工勾选实时反映到结果里
            for key, check in self.manual_checks.items():
                self._set(key, "pass" if check.isChecked() else "pending",
                          "已勾选" if check.isChecked() else "")
            return
        elapsed = time.monotonic() - self.started_at
        if elapsed >= 5.0 and self._find("comm")["state"] == "running":
            rate = 1.0 / (sum(self.tel_intervals) / len(self.tel_intervals)) if self.tel_intervals else 0.0
            ok = rate >= 20.0
            self._set("comm", "pass" if ok else "fail", f"实测 {rate:.1f} Hz")
        if elapsed >= 5.0 and self._find("battery")["state"] == "running":
            if self.battery_min is None:
                self._set("battery", "skip", "遥测中没有 battery 字段")
            else:
                ok = self.battery_min >= self.warn_voltage
                self._set("battery", "pass" if ok else "fail", f"最低 {self.battery_min:.2f} V")
        if elapsed >= 4.5 and self._find("imu")["state"] == "running":
            if not self.gyro_samples:
                self._set("imu", "skip", "遥测中没有 gyro_z 字段")
            else:
                mean = sum(self.gyro_samples) / len(self.gyro_samples)
                ok = abs(mean) < 2.0
                self._set("imu", "pass" if ok else "fail",
                          f"均值 {mean:+.2f} °/s（{len(self.gyro_samples)} 样本）")
        if elapsed >= 3.0 and self._find("fw")["state"] == "pending":
            if self.fw_version:
                self._set("fw", "pass", self.fw_version)
            else:
                self._set("fw", "fail", "1.5s 内未收到 fw_version NOTE（旧固件？）")
        if elapsed >= 6.0:
            self.running = False
            for key, check in self.manual_checks.items():
                self._set(key, "pass" if check.isChecked() else "pending",
                          "已勾选" if check.isChecked() else "未勾选")
            failed = [i["name"] for i in self._items if i["state"] == "fail"]
            self.status.setText(
                "自动检查完成。" + ("未通过：" + "、".join(failed) if failed else "自动项全部通过，请完成人工勾选后生成报告。"))

    def _on_fw_version(self, version):
        self.fw_version = version

    def _report(self):
        items = [dict(item, state=("pass" if (item["kind"] == "manual" and self.manual_checks[item["key"]].isChecked())
                                   else item["state"]))
                 for item in self._items]
        vehicle = self.config.get("vehicle", {}).get("display_name", "未指定车型")
        html = build_qc_report_html(vehicle, items, self.fw_version, self.operator.text().strip())
        reports = data_root() / "reports"
        reports.mkdir(parents=True, exist_ok=True)
        path = reports / f"qc_{time.strftime('%Y%m%d_%H%M%S')}.html"
        path.write_text(html, encoding="utf-8")
        QDesktopServices.openUrl(QUrl.fromLocalFile(str(path)))
        self.status.setText(f"报告已生成：{path}")
