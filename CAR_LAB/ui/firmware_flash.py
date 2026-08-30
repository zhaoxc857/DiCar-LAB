from pathlib import Path

from PySide6.QtCore import QProcess, Qt, QThread, Signal
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

from core import mspm0_bsl
from core.flash_job import FlashJobState, FlashState

IDLE_MESSAGES = {
    FlashState.IDLE: "就绪",
    FlashState.SUCCEEDED: "烧录成功",
}

# Per-family bootloader guidance. STM32 families share the AN3155 USART ROM
# bootloader (8E1, baud autodetect) flashed via stm32flash; MSPM0 uses the
# TI ROM BSL (9600 8N1 fixed) driven by the built-in core.mspm0_bsl driver.
DEFAULT_FLASH_FAMILY = "STM32F1"
FLASH_GUIDANCE = {
    "STM32F1": (
        "无线烧录步骤：\n"
        "1. 点击上方「断开」，释放串口；\n"
        "2. 车断电，BOOT0 跳线帽挪到 1，重新上电（OLED 熄灭属正常）；\n"
        "3. 回到本页选择固件并点击「开始烧录」；\n"
        "4. 烧录完成后断电，BOOT0 挪回 0，再上电即可运行新固件。\n"
        "烧录期间车辆主控不运行，电机不会启动。"
    ),
    "STM32F4": (
        "无线烧录步骤（STM32F4）：\n"
        "1. 点击上方「断开」，释放串口；\n"
        "2. 车断电，BOOT0 跳线帽接到 VDD，重新上电进入系统 bootloader；\n"
        "   F4 bootloader 以 USART1（PA9/PA10）为主接口，部分型号还支持\n"
        "   USART3（PB10/PB11）等，具体以 AN2606 对应型号的表格为准；\n"
        "   蓝牙模块需保持 115200-8E1（与 F103 相同，已用 AT 持久化配置则无需重配）；\n"
        "3. 回到本页选择固件并点击「开始烧录」；\n"
        "4. F4 采用扇区擦除，大容量芯片的擦除加写入可能需要几分钟，\n"
        "   属正常现象，请勿中途取消；\n"
        "5. 烧录完成后断电，BOOT0 挪回 0，再上电即可运行新固件。\n"
        "握手失败排查：确认 BOOT0 已接 VDD 且重新上电；个别型号 bootloader\n"
        "对高波特率探测受限，可尝试把波特率降到 9600 后重试。"
    ),
    "MSPM0G3507": (
        "无线烧录步骤（TI MSPM0G3507，未实板验证）：\n"
        "1. 点击上方「断开」，释放串口；\n"
        "2. 让车辆进入 BSL：开发板按 BSL 键（BSL Invoke，默认 PA18 拉低）\n"
        "   并复位；固件若实现了 PREPARE_FLASH 则可由上位机软触发（后续开放）；\n"
        "   ROM BSL 固定走 UART0：PA10=BSL_RX、PA11=BSL_TX（SLAU887）；\n"
        "3. 蓝牙模块必须配置为 9600-8N1（AT+UART=9600,0,0），\n"
        "   注意与 STM32 车的 115200-8E1 不同；\n"
        "4. 回到本页选择固件（.bin，≤128KB）并点击「开始烧录」；\n"
        "   9600 波特率下 128KB 约需 3~5 分钟，请勿中途取消；\n"
        "5. 烧录完成后应用自动启动，无需复位。\n"
        "握手失败排查：确认已进 BSL（再按 BSL 键复位一次）、蓝牙为 9600-8N1、\n"
        "接线为 PA10/PA11 交叉（RX↔TX）。"
    ),
}

# Flash backend per chip family: stm32flash.exe subprocess vs the built-in
# Python TI ROM BSL driver.
MSPM0_FAMILY = "MSPM0G3507"
STM32_FAMILIES = ("STM32F1", "STM32F4")


class Mspm0FlashWorker(QThread):
    log_line = Signal(str)
    finished_with_code = Signal(int)

    def __init__(self, port: str, firmware_path: str, parent=None):
        super().__init__(parent)
        self.port = port
        self.firmware_path = firmware_path
        self.cancelled = False

    def cancel(self):
        self.cancelled = True

    def run(self):
        try:
            image = Path(self.firmware_path).read_bytes()
        except OSError as exc:
            self.log_line.emit(f"读取固件失败：{exc}")
            self.finished_with_code.emit(1)
            return
        if len(image) > mspm0_bsl.G3507_MAIN_FLASH_SIZE:
            self.log_line.emit("固件超出 G3507 主闪存 128KB 上限。")
            self.finished_with_code.emit(1)
            return
        try:
            import serial

            ser = serial.Serial(
                self.port, 9600, bytesize=8, parity="N", stopbits=1, timeout=15
            )
        except Exception as exc:  # noqa: BLE001 - surface any open failure
            self.log_line.emit(f"打开串口失败：{exc}")
            self.finished_with_code.emit(1)
            return
        try:
            mspm0_bsl.flash_image(
                ser,
                image,
                should_continue=lambda: not self.cancelled,
                progress=self._on_progress,
                log=self.log_line.emit,
            )
            self.finished_with_code.emit(0)
        except mspm0_bsl.BslError as exc:
            if isinstance(exc, mspm0_bsl.BslCancelled):
                self.log_line.emit("=== 已取消。===")
            else:
                self.log_line.emit(f"=== 烧录失败（{exc.kind}）：{exc.detail} ===")
            self.finished_with_code.emit(1)
        except Exception as exc:  # noqa: BLE001 - keep the GUI alive
            self.log_line.emit(f"=== 烧录失败：{exc} ===")
            self.finished_with_code.emit(1)
        finally:
            try:
                ser.close()
            except Exception:  # noqa: BLE001 - best effort cleanup
                pass

    def _on_progress(self, written: int, total: int):
        if written == total:
            self.log_line.emit(f"写入进度：{written}/{total} 字节（完成）")


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
            "通过 HC-05 蓝牙串口无线烧录固件：STM32 走 stm32flash，"
            "MSPM0 走内置 TI ROM BSL 驱动（未实板验证）。"
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
        family_row = QHBoxLayout()
        family_row.addWidget(QLabel("芯片系列"))
        self.family_combo = QComboBox()
        for item in FLASH_GUIDANCE:
            self.family_combo.addItem(item)
        default_family = str(config.get("flash", {}).get("family", DEFAULT_FLASH_FAMILY))
        if default_family not in FLASH_GUIDANCE:
            default_family = DEFAULT_FLASH_FAMILY
        self.family_combo.setCurrentText(default_family)
        self.family_combo.currentTextChanged.connect(self._on_family_changed)
        self.family_combo.setFixedWidth(120)
        family_row.addWidget(self.family_combo)
        family_row.addStretch(1)
        target_layout.addLayout(family_row)
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
        self.log.setPlainText(self._guidance_text())
        root.addWidget(self.log, 1)

        safety = QLabel(
            "安全要求：烧录前自动断开车辆连接；任何失败都会停止任务。"
        )
        safety.setWordWrap(True)
        safety.setObjectName("muted")
        root.addWidget(safety)

        self.worker = None
        self._on_family_changed()
        self.run_button.clicked.connect(self._start_flash)

    def _backend_available(self):
        if self.family_combo.currentText() == MSPM0_FAMILY:
            return True
        return bool(self.flash_backend)

    def _guidance_text(self):
        return FLASH_GUIDANCE.get(
            self.family_combo.currentText(), FLASH_GUIDANCE[DEFAULT_FLASH_FAMILY]
        )

    def _on_family_changed(self):
        is_mspm0 = self.family_combo.currentText() == MSPM0_FAMILY
        # TI ROM BSL is fixed at 9600 8N1; lock the selector for MSPM0.
        self.baud_combo.setCurrentText("9600")
        self.baud_combo.setEnabled(not is_mspm0)
        if self.state.state == FlashState.IDLE:
            self.log.setPlainText(self._guidance_text())
            if not self._backend_available():
                # STM32 family selected but no stm32flash backend. Built
                # directly - the state machine only models UNAVAILABLE ->
                # IDLE, availability is a UI-level concern.
                self.state = FlashJobState(FlashState.UNAVAILABLE, "烧录后端尚未配置")
                self._set_reason("烧录后端尚未配置")
                self.run_button.setEnabled(False)
        elif self.state.state == FlashState.UNAVAILABLE and self._backend_available():
            self.state = FlashJobState(FlashState.IDLE, "就绪")
            self._set_reason("就绪", good=True)
            self.run_button.setEnabled(True)

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
        if self.family_combo.currentText() == MSPM0_FAMILY:
            size = Path(firmware).stat().st_size
            if size > mspm0_bsl.G3507_MAIN_FLASH_SIZE:
                self._reject("固件超出 G3507 主闪存 128KB 上限，无法烧录")
                return
        self.state = self.state.transition(FlashState.VALIDATING, "校验烧录条件…")
        self._set_reason("校验烧录条件…")
        if self.transport is not None and self.transport.connected:
            self.transport.disconnect()
            self.log.appendPlainText("已自动断开车辆连接，释放串口。")
        if self.family_combo.currentText() == MSPM0_FAMILY:
            self._start_mspm0_worker(port, firmware)
            return
        self.state = self.state.transition(FlashState.FLASHING, "正在烧录…")
        self._set_reason("正在烧录…")
        command = build_command(self.flash_backend, port, int(self.baud_combo.currentText()), firmware)
        self.log.appendPlainText("$ " + " ".join(command))
        self.process = QProcess(self)
        self.process.readyReadStandardOutput.connect(self._on_output)
        self.process.finished.connect(self._on_finished)
        self.process.start(command[0], command[1:])

    def _start_mspm0_worker(self, port, firmware):
        self.state = self.state.transition(FlashState.FLASHING, "正在烧录…")
        self._set_reason("正在烧录…")
        self.log.appendPlainText(f"以 9600-8N1 连接 {port}，使用内置 TI ROM BSL 驱动。")
        self.worker = Mspm0FlashWorker(port, firmware, self)
        self.worker.log_line.connect(self.log.appendPlainText)
        self.worker.finished_with_code.connect(self._on_mspm0_finished)
        self.worker.start()

    def _cancel_flash(self):
        if self.state.state != FlashState.FLASHING:
            return
        if self.worker is not None:
            self.worker.cancel()
            self._set_reason("正在取消…")
        elif self.process is not None:
            self.process.kill()
            self._set_reason("正在取消…")

    def _on_mspm0_finished(self, code):
        self.worker = None
        if code == 0:
            self.state = self.state.transition(FlashState.VERIFYING, "写入完成，回读校验通过")
            self.state = self.state.transition(FlashState.SUCCEEDED, "烧录成功")
            self._set_reason("烧录成功", good=True)
            self.log.appendPlainText("=== 烧录成功，应用已由 BSL 启动。 ===")
        else:
            self.state = self.state.transition(FlashState.FAILED, "烧录失败，详见日志")
            self._set_reason("烧录失败，详见日志")
            self.log.appendPlainText(
                "=== 烧录失败。确认车辆已进 BSL、蓝牙为 9600-8N1、PA10/PA11 接线正确后重试。 ==="
            )
        self.state = self.state.transition(FlashState.IDLE)
        self._refresh_action_buttons()

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
        self.run_button.setEnabled(
            not active and self.state.state != FlashState.UNAVAILABLE
        )
        self.cancel_button.setEnabled(self.state.state == FlashState.FLASHING)


def build_command(exe, port, baud, firmware):
    from core.flash_backend import build_flash_command

    return build_flash_command(exe, port, baud, firmware)
