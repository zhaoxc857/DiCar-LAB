"""复盘回放页：录制遥测会话为 JSONL，再以 0.5~4x 速度回放到整条遥测总线，
让示波器、弯道分析、诊断全部对着历史数据重新跑一遍。"""

from pathlib import Path

from PySide6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton, QListWidget,
    QGroupBox, QComboBox, QProgressBar, QMessageBox,
)

from core.session_recorder import (
    ReplayPlayer, SessionRecorder, load_recording, recordings_dir,
)


class ReplayPage(QWidget):
    def __init__(self, bus, transport, config):
        super().__init__()
        self.bus = bus
        self.transport = transport
        self.recorder = SessionRecorder(bus, self)
        self.player = ReplayPlayer(bus, self)
        self._samples = []
        self._path = ""

        root = QVBoxLayout(self)
        root.setSpacing(10)

        record_box = QGroupBox("① 录制会话")
        row = QHBoxLayout(record_box)
        self.record_btn = QPushButton("开始录制")
        self.record_btn.setObjectName("primary")
        self.record_btn.clicked.connect(self._toggle_record)
        row.addWidget(self.record_btn)
        self.record_status = QLabel("录制会把收到的每一帧遥测写入 recordings/ 目录（JSONL，自动停止于 20 万帧）。")
        self.record_status.setObjectName("muted")
        self.record_status.setWordWrap(True)
        row.addWidget(self.record_status, 1)
        root.addWidget(record_box)

        list_box = QGroupBox("② 选择录制")
        list_layout = QVBoxLayout(list_box)
        self.list = QListWidget()
        self.list.itemDoubleClicked.connect(lambda _item: self._load_selected())
        list_layout.addWidget(self.list, 1)
        refresh = QPushButton("刷新列表")
        refresh.clicked.connect(self._refresh_list)
        list_layout.addWidget(refresh)
        root.addWidget(list_box, 1)

        play_box = QGroupBox("③ 回放（回放前建议先断开车辆连接）")
        play_layout = QHBoxLayout(play_box)
        self.play_btn = QPushButton("播放")
        self.play_btn.setObjectName("primary")
        self.play_btn.clicked.connect(self._toggle_play)
        self.stop_btn = QPushButton("停止")
        self.stop_btn.clicked.connect(self._stop)
        self.speed_combo = QComboBox()
        for s in ("0.5x", "1x", "2x", "4x"):
            self.speed_combo.addItem(s, float(s.rstrip("x")))
        self.speed_combo.setCurrentIndex(1)
        self.progress = QProgressBar()
        self.progress.setRange(0, 1000)
        play_layout.addWidget(self.play_btn)
        play_layout.addWidget(self.stop_btn)
        play_layout.addWidget(QLabel("速度"))
        play_layout.addWidget(self.speed_combo)
        play_layout.addWidget(self.progress, 1)
        root.addWidget(play_box)

        self.status = QLabel("就绪")
        self.status.setObjectName("muted")
        root.addWidget(self.status)
        self.player.tick.connect(self._on_tick)
        self.player.finished.connect(self._on_finished)
        self.recorder.stopped.connect(self._on_record_stopped)
        self._refresh_list()

    # -- recording ---------------------------------------------------------

    def _toggle_record(self):
        if self.recorder.recording:
            self.recorder.stop()
            return
        if self.transport is not None and self.transport.connected:
            answer = QMessageBox.question(
                self, "录制会话",
                "车辆当前处于连接状态，录制会保存实时遥测。继续吗？")
            if answer != QMessageBox.StandardButton.Yes:
                return
        path = self.recorder.start()
        self.record_btn.setText("停止录制")
        self.record_status.setText(f"录制中 → {Path(path).name}")

    def _on_record_stopped(self, path, count):
        self.record_btn.setText("开始录制")
        self.record_status.setText(f"已保存 {count} 帧 → {path}")
        self._refresh_list()

    def _refresh_list(self):
        self.list.clear()
        for path in sorted(recordings_dir().glob("session_*.jsonl"), reverse=True):
            self.list.addItem(str(path))

    # -- playback ------------------------------------------------------------

    def _selected_path(self):
        item = self.list.currentItem()
        return item.text() if item is not None else ""

    def _load_selected(self):
        path = self._selected_path()
        if not path:
            self.status.setText("请先在列表中选择一个录制文件（双击也可载入）。")
            return None
        try:
            self._samples = load_recording(path)
        except OSError as exc:
            self.status.setText(f"读取失败：{exc}")
            return None
        self._path = path
        self.player.load(self._samples)
        self.status.setText(
            f"已载入 {Path(path).name}：{len(self._samples)} 帧，时长 {self.player.duration:.1f}s")
        return path

    def _toggle_play(self):
        if self.player.playing:
            self.player.pause()
            self.play_btn.setText("继续")
            return
        if not self.player.samples and not self._load_selected():
            return
        if self.player.cursor >= len(self.player.samples):
            self.player.load(self._samples)
        self.player.play(self.speed_combo.currentData())
        self.play_btn.setText("暂停")

    def _stop(self):
        self.player.stop()
        self.play_btn.setText("播放")
        self.progress.setValue(0)
        self.status.setText("回放已停止。")

    def _on_tick(self, _data):
        if self.player.duration > 0:
            elapsed = min(1.0, self.player.cursor / max(1, len(self.player.samples)))
            self.progress.setValue(int(elapsed * 1000))

    def _on_finished(self):
        self.play_btn.setText("播放")
        self.progress.setValue(1000)
        self.status.setText(f"回放结束：{Path(self._path).name}")
