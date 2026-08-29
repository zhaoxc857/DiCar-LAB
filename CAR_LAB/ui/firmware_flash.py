from pathlib import Path

from PySide6.QtCore import QProcess, Qt
from PySide6.QtWidgets import (
    QComboBox,
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

from core.flash_job import FlashJobState, FlashState

IDLE_MESSAGES = {
    FlashState.IDLE: "就绪",
    FlashState.SUCCEEDED: "烧录成功",
}


class FirmwareFlashPage(QWidget):
    """Single-shot wireless flashing over the HC-05 serial link.

    The backend shells out to the bundled stm32flash.exe; the exact
    command is built by core.flash_backend.build_flash_command. The car
    must be placed in bootloader mode (BOOT0 jumper to 1, then power
    cycle) before starting; the transport connection is dropped first
    because both features need exclusive access to the COM port.
    """

    def __init__(self, config, transport=None, flash_backend=None):
        super().__init__()
        self.state = FlashJobState()
        self.transport = transport
        self.flash_backend = flash_backend
        self.process = None
        self._firmware_path = ""

        transport_cfg = config.get("transport", {})
        default_port = str(transport_cfg.get("port", "COM6"))
        default_baud = str(transport_cfg.get("baudrate", 9600))

        root = QVBoxLayout(self)
        root.setSpacing(12)

        intro = QLabel(
            "通过 HC-05 蓝牙串口无线烧录 STM32 固件（stm32flash 后端）。"
        )
        intro.setWordWrap(True)
        intro.setObjectName("muted")
        root.addWidget(intro)

        target_box = QGroupBox("目标设备")
        target_layout = QVBoxLayout(target_box)
        vehicle_name = config.get("vehicle", {}).get("display_name", "当前车型")
        self.target_label = QLabel(f"当前车型：{vehicle_name}")
        target_layout.addWidget(self.target_label)
        self.backend_label = QLabel(
            f"后端：{flash_backend}" if flash_backend else "后端状态：未检测到已验证的烧录器"
        )
        target_layout.addWidget(self.backend_label)
        port_row = QHBoxLayout()
        port_row.addWidget(QLabel("串口"))
        self.port_edit = QLineEdit(default_port)
        self.port_edit.setFixedWidth(90)
        port_row.addWidget(self.port_edit)
        port_row.addWidget(QLabel("波特率"))
        self.baud_combo = QComboBox()
        for item in ("9600", "115200"):
            self.baud_combo.addItem(item)
        self.baud_combo.setCurrentText(default_baud if default_baud in ("9600", "115200") else "9600")
        self.baud_combo.setFixedWidth(90)
        port_row.addWidget(self.baud_combo)
        port_row.addStretch(1)
        target_layout.addLayout(port_row)
        root.addWidget(target_box)

        firmware_box = QGroupBox("固件与模式")
        firmware_layout = QVBoxLayout(firmware_box)
        path_row = QHBoxLayout()
        self.firmware_path = QLineEdit()
        self.firmware_path.setReadOnly(True)
        self.firmware_path.setPlaceholderText("选择要烧录的固件文件")
        browse_button = QPushButton("选择固件")
        browse_button.clicked.connect(self._choose_firmware)
        path_row.addWidget(self.firmware_path, 1)
        path_row.addWidget(browse_button)
        firmware_layout.addLayout(path_row)

        mode_row = QHBoxLayout()
        self.single_mode = QRadioButton("单次烧录")
        self.single_mode.setChecked(True)
        self.continuous_mode = QRadioButton("连续烧录")
        self.continuous_mode.setEnabled(False)
        self.continuous_mode.setToolTip("连续烧录将在单次模式稳定后开放")
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
        action_row.addWidget(self.reason_label)
        action_row.addStretch(1)
        self.cancel_button = QPushButton("取消")
        self.cancel_button.setEnabled(False)
        self.cancel_button.clicked.connect(self._cancel_flash)
        action_row.addWidget(self.cancel_button)
        action_row.addWidget(self.run_button)
        root.addLayout(action_row)

        self.log = QPlainTextEdit()
        self.log.setReadOnly(True)
        self.log.setPlainText(
            "无线烧录步骤：\n"
            "1. 点击上方「断开」，释放串口；\n"
            "2. 车断电，BOOT0 跳线帽挪到 1，重新上电（OLED 熄灭属正常）；\n"
            "3. 回到本页选择固件并点击「开始烧录」；\n"
            "4. 烧录完成后断电，BOOT0 挪回 0，再上电即可运行新固件。\n"
            "烧录期间车辆主控不运行，电机不会启动。"
        )
        root.addWidget(self.log, 1)

        safety = QLabel(
            "安全要求：烧录前自动断开车辆连接；任何失败都会停止任务。"
        )
        safety.setWordWrap(True)
        safety.setObjectName("muted")
        root.addWidget(safety)

        if flash_backend:
            self.state = FlashJobState(FlashState.IDLE, "就绪")
            self.reason_label.setText("就绪")
            self.reason_label.setObjectName("statusGood")
            self.run_button.setEnabled(True)
        self.run_button.clicked.connect(self._start_flash)

    def _choose_firmware(self):
        path, _ = QFileDialog.getOpenFileName(
            self,
            "选择固件",
            "",
            "Firmware (*.hex *.bin);;All files (*)",
        )
        if path:
            self.firmware_path.setText(path)

    def _set_reason(self, message, good=False):
        self.reason_label.setText(message)
        self.reason_label.setObjectName("statusGood" if good else "statusBad")
        self.reason_label.style().unpolish(self.reason_label)
        self.reason_label.style().polish(self.reason_label)

    def _transition(self, target, message=""):
        self.state = self.state.transition(target, message)
        self._set_reason(message or self.state.message, target == FlashState.SUCCEEDED)
        active = self.state.state in (
            FlashState.VALIDATING,
            FlashState.FLASHING,
            FlashState.VERIFYING,
        )
        self.run_button.setEnabled(not active)
        self.cancel_button.setEnabled(self.state.state == FlashState.FLASHING)

    def _reject(self, message):
        """Abort preflight: IDLE -> VALIDATING -> FAILED -> IDLE."""
        self.state = self.state.transition(FlashState.VALIDATING)
        self.state = self.state.transition(FlashState.FAILED, message)
        self.state = self.state.transition(FlashState.IDLE)
        self._set_reason(message)

    def _start_flash(self):
        if self.state.state in (
            FlashState.VALIDATING,
            FlashState.FLASHING,
            FlashState.VERIFYING,
        ):
            return
        if self.state.state == FlashState.SUCCEEDED:
            self.state = self.state.transition(FlashState.IDLE, "就绪")
        port = self.port_edit.text().strip()
        if not port:
            self._reject("未填写串口端口")
            return
        firmware = self.firmware_path.text().strip()
        if not firmware or not Path(firmware).is_file():
            self._reject("固件文件不存在，请重新选择")
            return
        self.state = self.state.transition(FlashState.VALIDATING, "校验烧录条件…")
        self._set_reason("校验烧录条件…")
        if self.transport is not None and self.transport.connected:
            self.transport.disconnect()
            self.log.appendPlainText("已自动断开车辆连接，释放串口。")
        self.state = self.state.transition(FlashState.FLASHING, "正在烧录…")
        self._set_reason("正在烧录…")
        command = build_command(self.flash_backend, port, int(self.baud_combo.currentText()), firmware)
        self.log.appendPlainText("$ " + " ".join(command))
        self.process = QProcess(self)
        self.process.readyReadStandardOutput.connect(self._on_output)
        self.process.finished.connect(self._on_finished)
        self.process.start(command[0], command[1:])

    def _cancel_flash(self):
        if self.process is not None and self.state.state == FlashState.FLASHING:
            self.process.kill()
            self._set_reason("正在取消…")

    def _on_output(self):
        if self.process is None:
            return
        text = bytes(self.process.readAllStandardOutput()).decode(
            "utf-8", errors="replace"
        ).strip()
        if text:
            for line in text.splitlines():
                self.log.appendPlainText(line)

    def _on_finished(self, code, _status):
        self.process = None
        if code == 0:
            self.state = self.state.transition(FlashState.VERIFYING, "写入完成，回读校验通过")
            self.state = self.state.transition(FlashState.SUCCEEDED, "烧录成功")
            self._set_reason("烧录成功", good=True)
            self.log.appendPlainText("=== 烧录成功。请断电将 BOOT0 挪回 0 后重启车辆。 ===")
        else:
            self.state = self.state.transition(FlashState.FAILED, f"烧录失败（退出码 {code}），详见日志")
            self._set_reason(f"烧录失败（退出码 {code}），详见日志")
            self.log.appendPlainText("=== 烧录失败。检查 BOOT0 是否在 1、串口是否被占用后重试。 ===")
        self.state = self.state.transition(FlashState.IDLE)
        self._refresh_action_buttons()

    def _refresh_action_buttons(self):
        active = self.state.state in (
            FlashState.VALIDATING,
            FlashState.FLASHING,
            FlashState.VERIFYING,
        )
        self.run_button.setEnabled(not active)
        self.cancel_button.setEnabled(self.state.state == FlashState.FLASHING)


def build_command(exe, port, baud, firmware):
    from core.flash_backend import build_flash_command

    return build_flash_command(exe, port, baud, firmware)
