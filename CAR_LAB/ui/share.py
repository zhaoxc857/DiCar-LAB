"""赛道分享页：把遥测总线以只读 SSE 服务共享给局域网设备（手机/平板浏览器）。"""

from PySide6.QtCore import QTimer
from PySide6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton, QSpinBox,
    QGroupBox, QListWidget, QMessageBox,
)

from core.telemetry_server import TelemetryServer


class SharePage(QWidget):
    def __init__(self, bus, transport, config):
        super().__init__()
        self.bus = bus
        self.transport = transport
        self.server = TelemetryServer()

        root = QVBoxLayout(self)
        root.setSpacing(10)

        box = QGroupBox("只读遥测分享（SSE，无新增依赖）")
        layout = QVBoxLayout(box)
        row = QHBoxLayout()
        row.addWidget(QLabel("端口"))
        self.port = QSpinBox()
        self.port.setRange(1024, 65535)
        self.port.setValue(8899)
        row.addWidget(self.port)
        self.toggle_btn = QPushButton("启动分享")
        self.toggle_btn.setObjectName("primary")
        self.toggle_btn.clicked.connect(self._toggle)
        row.addWidget(self.toggle_btn)
        row.addStretch(1)
        layout.addLayout(row)

        self.urls = QListWidget()
        self.urls.setMaximumHeight(90)
        layout.addWidget(self.urls)
        self.status = QLabel("未启动。手机与电脑需在同一局域网；首次启动 Windows 可能弹出防火墙询问，选择「允许」。")
        self.status.setObjectName("muted")
        self.status.setWordWrap(True)
        layout.addWidget(self.status)
        root.addWidget(box)

        note = QGroupBox("安全边界")
        note_layout = QVBoxLayout(note)
        hint = QLabel(
            "本服务严格只读：仅暴露遥测曲线，不提供任何控制入口；\n"
            "急停只在上位机本体上。同一局域网内的任何人都能看到数据，\n"
            "请勿在公共网络中使用。停止分享后服务立即关闭。"
        )
        hint.setWordWrap(True)
        note_layout.addWidget(hint)
        root.addWidget(note)
        root.addStretch(1)

        bus.telemetry.connect(self._tel)
        self._timer = QTimer(self)
        self._timer.setInterval(1000)
        self._timer.timeout.connect(self._tick)
        self._timer.start(1000)

    def _toggle(self):
        if self.server.running:
            self.server.stop()
            self.toggle_btn.setText("启动分享")
            self.urls.clear()
            self.status.setText("已停止。")
            return
        try:
            self.server.port = self.port.value()
            self.server.start()
        except OSError as exc:
            QMessageBox.warning(self, "赛道分享", f"端口被占用或无法监听：{exc}")
            return
        self.toggle_btn.setText("停止分享")
        self.urls.clear()
        for url in TelemetryServer.local_urls(self.port.value()):
            self.urls.addItem(url)
        self.status.setText("分享中 · 用手机浏览器打开上方任一地址即可实时查看曲线。")

    def _tel(self, data):
        if self.server.running:
            self.server.state.publish(dict(data))

    def _tick(self):
        if self.server.running:
            clients = self.server.state.client_count
            self.status.setText(
                f"分享中 · {self.port.value()} 端口 · {clients} 个浏览器连接。")
