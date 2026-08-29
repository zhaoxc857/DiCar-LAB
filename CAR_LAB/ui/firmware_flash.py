from PySide6.QtWidgets import (
    QFileDialog,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPlainTextEdit,
    QPushButton,
    QRadioButton,
    QVBoxLayout,
    QWidget,
)

from core.flash_job import FlashJobState


class FirmwareFlashPage(QWidget):
    def __init__(self, config):
        super().__init__()
        self.state = FlashJobState()

        root = QVBoxLayout(self)
        root.setSpacing(12)

        intro = QLabel(
            "本页为后续 STM32 单次与连续烧录预留安全工作流。"
            "当前版本不会调用任何烧录工具。"
        )
        intro.setWordWrap(True)
        intro.setObjectName("muted")
        root.addWidget(intro)

        target_box = QGroupBox("目标设备")
        target_layout = QVBoxLayout(target_box)
        vehicle_name = config.get("vehicle", {}).get("display_name", "当前车型")
        self.target_label = QLabel(f"当前车型：{vehicle_name}")
        target_layout.addWidget(self.target_label)
        target_layout.addWidget(QLabel("后端状态：未检测到已验证的烧录器"))
        root.addWidget(target_box)

        firmware_box = QGroupBox("固件与模式")
        firmware_layout = QVBoxLayout(firmware_box)
        path_row = QHBoxLayout()
        self.firmware_path = QLineEdit()
        self.firmware_path.setReadOnly(True)
        self.firmware_path.setPlaceholderText("选择固件文件，仅记录路径，不执行烧录")
        browse_button = QPushButton("选择固件")
        browse_button.clicked.connect(self._choose_firmware)
        path_row.addWidget(self.firmware_path, 1)
        path_row.addWidget(browse_button)
        firmware_layout.addLayout(path_row)

        mode_row = QHBoxLayout()
        self.single_mode = QRadioButton("单次烧录")
        self.single_mode.setChecked(True)
        self.continuous_mode = QRadioButton("连续烧录")
        mode_row.addWidget(self.single_mode)
        mode_row.addWidget(self.continuous_mode)
        mode_row.addStretch(1)
        firmware_layout.addLayout(mode_row)
        root.addWidget(firmware_box)

        action_row = QHBoxLayout()
        self.reason_label = QLabel(self.state.message)
        self.reason_label.setObjectName("statusBad")
        self.run_button = QPushButton("开始烧录")
        self.run_button.setObjectName("primary")
        self.run_button.setEnabled(False)
        self.run_button.setToolTip(self.state.message)
        self.run_button.setAccessibleDescription(self.state.message)
        action_row.addWidget(self.reason_label)
        action_row.addStretch(1)
        action_row.addWidget(self.run_button)
        root.addLayout(action_row)

        self.log = QPlainTextEdit()
        self.log.setReadOnly(True)
        self.log.setPlainText(
            "本版本仅预留安全烧录边界，未加载任何烧录后端。\n"
            "后续实现必须依次完成固件校验、目标确认、烧录和写后验证；"
            "任何失败都会停止连续任务。"
        )
        root.addWidget(self.log, 1)

        safety = QLabel(
            "安全要求：烧录期间电机不得启动；连续模式必须支持失败即停和安全取消。"
        )
        safety.setWordWrap(True)
        safety.setObjectName("muted")
        root.addWidget(safety)

    def _choose_firmware(self):
        path, _ = QFileDialog.getOpenFileName(
            self,
            "选择固件",
            "",
            "Firmware (*.bin *.hex *.elf);;All files (*)",
        )
        if path:
            self.firmware_path.setText(path)
