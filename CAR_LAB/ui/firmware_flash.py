"""Wireless firmware flashing: STM32 via stm32flash, MSPM0 via built-in
TI ROM BSL driver, with a visual progress bar, cancellable stages, and a
firmware version library (per-flash snapshots, notes and one-click rollback).

Progress design: the log area only carries phase and error lines; write
progress drives the progress bar. MSPM0 reports stages/percent directly
from the driver thread; stm32flash emits carriage-return progress lines on
stdout which are parsed and filtered here.
"""

import time
from pathlib import Path

from PySide6.QtCore import QProcess, QSettings, QThread, QUrl, Signal
from PySide6.QtGui import QDesktopServices
from PySide6.QtWidgets import (
    QAbstractItemView,
    QComboBox,
    QFileDialog,
    QGroupBox,
    QHBoxLayout,
    QHeaderView,
    QInputDialog,
    QLabel,
    QLineEdit,
    QMenu,
    QMessageBox,
    QPlainTextEdit,
    QProgressBar,
    QPushButton,
    QRadioButton,
    QTableWidget,
    QTableWidgetItem,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from core import mspm0_bsl
from core.flash_backend import (
    DEFAULT_SERIAL_MODE,
    build_flash_command,
    check_firmware_size,
    classify_output_segment,
    split_output_segments,
)
from core.flash_job import FlashJobState, FlashState
from core.firmware_store import (
    RESULT_CANCELLED,
    RESULT_FAILED,
    RESULT_OK,
    FirmwareStore,
)
from core.ports import list_serial_ports

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
        "串口格式：上位机已显式按 8E1（偶校验）打开串口；蓝牙模块与 MCU\n"
        "直连的 UART 必须同为 115200-8E1（HC-05：AT+UART=115200,0,2）。\n"
        "烧录期间车辆主控不运行，电机不会启动。"
    ),
    "STM32F4": (
        "无线烧录步骤（STM32F4）：\n"
        "1. 点击上方「断开」，释放串口；\n"
        "2. 车断电，BOOT0 跳线帽接到 VDD，重新上电进入系统 bootloader；\n"
        "   F4 bootloader 以 USART1（PA9/PA10）为主接口，部分型号还支持\n"
        "   USART3（PB10/PB11）等，具体以 AN2606 对应型号的表格为准；\n"
        "   蓝牙模块需保持 115200-8E1（AT+UART=115200,0,2，与 F103 相同）；\n"
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
        "   该 9600 仅为 BSL 烧录链路约束；日常调参链路可把蓝牙 AT 配置\n"
        "   改高（如 115200），并把车型 YAML 的 transport.baudrate 一并修改；\n"
        "4. 回到本页选择固件（.bin，≤128KB）并点击「开始烧录」；\n"
        "   9600 波特率下整片擦除加写入约需 3~5 分钟，请勿中途取消；\n"
        "5. 烧录完成后应用自动启动，无需复位。\n"
        "握手失败排查：确认已进 BSL（再按 BSL 键复位一次）、蓝牙为 9600-8N1、\n"
        "接线为 PA10/PA11 交叉（RX↔TX）。"
    ),
}

# Flash backend per chip family: stm32flash.exe subprocess vs the built-in
# Python TI ROM BSL driver.
MSPM0_FAMILY = "MSPM0G3507"
STM32_FAMILIES = ("STM32F1", "STM32F4")

SERIAL_MODES = (
    ("8E1（偶校验 · AN3155 标准）", "8e1"),
    ("8N1（无校验 · 诊断用）", "8n1"),
)

MSPM0_STAGE_TEXTS = {
    "connecting": "连接 BSL…",
    "erasing": "擦除主闪存…",
    "programming": "写入固件…",
    "verifying": "回读校验…",
    "starting": "启动应用…",
}

RESULT_TEXT = {
    "pending": "进行中",
    RESULT_OK: "成功",
    RESULT_CANCELLED: "已取消",
    RESULT_FAILED: "失败",
}


class Mspm0FlashWorker(QThread):
    log_line = Signal(str)
    progress = Signal(int, int)
    stage = Signal(str)
    finished_with_code = Signal(int)

    def __init__(self, port: str, firmware_path: str, parent=None):
        super().__init__(parent)
        self.port = port
        self.firmware_path = firmware_path
        self.cancelled = False
        self.was_cancelled = False

    def cancel(self):
        self.cancelled = True

    def run(self):
        try:
            image = Path(self.firmware_path).read_bytes()
        except OSError as exc:
            self.log_line.emit(f"读取固件失败：{exc}")
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
                progress=lambda written, total: self.progress.emit(written, total),
                stage=lambda name: self.stage.emit(name),
                log=self.log_line.emit,
            )
            self.finished_with_code.emit(0)
        except mspm0_bsl.BslError as exc:
            if isinstance(exc, mspm0_bsl.BslCancelled):
                self.was_cancelled = True
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


class FirmwareFlashPage(QWidget):
    """Single-shot wireless flashing over the HC-05 serial link.

    STM32 flashing shells out to the bundled stm32flash.exe (command built
    by core.flash_backend.build_flash_command); MSPM0 uses the built-in TI
    ROM BSL driver on a worker thread. The car must be placed in bootloader
    mode (BOOT0 jumper / BSL key) before starting; the transport connection
    is dropped first because both features need exclusive access to the COM
    port. Every attempt is snapshotted into the firmware version library
    (core.firmware_store) with an optional user note for later rollback.
    """

    def __init__(self, config, transport=None, flash_backend=None, firmware_store=None):
        super().__init__()
        self.state = FlashJobState()
        self.transport = transport
        self.flash_backend = flash_backend
        self.process = None
        self._firmware_path = ""
        self._cancel_requested = False
        self._stm_pending = ""
        self._active_version_id = None
        self._library_rows = []
        self.config = config
        self.vehicle_id = str(config.get("vehicle", {}).get("id", "default"))
        self.settings = QSettings("DiCAR", "DiCAR LAB")
        try:
            self.firmware_store = firmware_store if firmware_store is not None else FirmwareStore()
        except Exception:  # noqa: BLE001 - a read-only data dir must not kill the page
            self.firmware_store = None

        transport_cfg = config.get("transport", {})
        default_port = str(transport_cfg.get("port", "COM6"))
        default_baud = str(transport_cfg.get("baudrate", 9600))

        root = QVBoxLayout(self)
        root.setSpacing(12)

        intro = QLabel(
            "通过 HC-05 蓝牙串口无线烧录固件：STM32 走 stm32flash，"
            "MSPM0 走内置 TI ROM BSL 驱动（未实板验证）。每次烧录自动存入固件版本库，可随时回退。"
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
        # 芯片族默认值由车型 YAML 的 flash.family 提供（随车型持久化），
        # 不再重复写 QSettings，避免跨车型串扰；端口/波特率/串口格式才走 QSettings。
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
        port_row.addWidget(self._make_port_menu_button())
        port_row.addWidget(QLabel("波特率"))
        self.baud_combo = QComboBox()
        for item in ("9600", "115200"):
            self.baud_combo.addItem(item)
        saved_baud = str(self.settings.value("flash/baud", "") or "")
        self.baud_combo.setCurrentText(saved_baud if saved_baud in ("9600", "115200") else "9600")
        if default_baud in ("9600", "115200") and not saved_baud:
            self.baud_combo.setCurrentText(default_baud)
        self.baud_combo.setFixedWidth(90)
        port_row.addWidget(self.baud_combo)
        port_row.addWidget(QLabel("串口格式"))
        self.mode_combo = QComboBox()
        for text, data in SERIAL_MODES:
            self.mode_combo.addItem(text, data)
        saved_mode = str(self.settings.value("flash/serial_mode", "") or "")
        mode_index = self.mode_combo.findData(saved_mode)
        self.mode_combo.setCurrentIndex(mode_index if mode_index >= 0 else 0)
        self.mode_combo.currentIndexChanged.connect(
            lambda _: self.settings.setValue("flash/serial_mode", self._serial_mode())
        )
        self.mode_combo.setFixedWidth(200)
        port_row.addWidget(self.mode_combo)
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
        note_row = QHBoxLayout()
        note_row.addWidget(QLabel("固件备注"))
        self.note_edit = QLineEdit()
        self.note_edit.setPlaceholderText("如：v3 加了转向环（留空自动用 文件名 @ 时间）")
        note_row.addWidget(self.note_edit, 1)
        firmware_layout.addLayout(note_row)

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

        progress_row = QHBoxLayout()
        self.stage_label = QLabel("就绪")
        self.stage_label.setObjectName("muted")
        self.progress_bar = QProgressBar()
        self.progress_bar.setRange(0, 100)
        self.progress_bar.setValue(0)
        self.progress_bar.setFixedWidth(340)
        progress_row.addWidget(self.stage_label, 1)
        progress_row.addWidget(self.progress_bar)
        root.addLayout(progress_row)

        root.addWidget(self._build_library_box())

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
        self._refresh_library()

    # -- UI construction helpers -------------------------------------------

    def _make_port_menu_button(self):
        button = QToolButton()
        button.setText("▾")
        button.setToolTip("扫描本机可用串口")
        menu = QMenu(button)
        menu.aboutToShow.connect(lambda: self._fill_port_menu(menu))
        button.setMenu(menu)
        button.setPopupMode(QToolButton.InstantPopup)
        return button

    def _fill_port_menu(self, menu):
        menu.clear()
        ports = list_serial_ports()
        if not ports:
            empty = menu.addAction("未发现串口设备")
            empty.setEnabled(False)
            return
        for device in ports:
            menu.addAction(device, lambda d=device: self.port_edit.setText(d))

    def _build_library_box(self):
        box = QGroupBox("固件版本库（每次烧录自动快照，可回退）")
        layout = QVBoxLayout(box)
        self.library_table = QTableWidget(0, 5)
        self.library_table.setHorizontalHeaderLabels(["时间", "备注", "大小", "结果", "来源"])
        self.library_table.horizontalHeader().setSectionResizeMode(1, QHeaderView.ResizeMode.Stretch)
        self.library_table.horizontalHeader().setStretchLastSection(True)
        self.library_table.verticalHeader().setVisible(False)
        self.library_table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.library_table.setSelectionMode(QAbstractItemView.SelectionMode.SingleSelection)
        self.library_table.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self.library_table.setMaximumHeight(150)
        layout.addWidget(self.library_table)
        button_row = QHBoxLayout()
        refresh_btn = QPushButton("刷新")
        refresh_btn.clicked.connect(self._refresh_library)
        rollback_btn = QPushButton("烧录此版本（回退）")
        rollback_btn.setObjectName("primary")
        rollback_btn.clicked.connect(self._rollback_selected)
        rename_btn = QPushButton("改备注")
        rename_btn.clicked.connect(self._rename_selected)
        delete_btn = QPushButton("删除")
        delete_btn.clicked.connect(self._delete_selected)
        open_btn = QPushButton("打开快照目录")
        open_btn.clicked.connect(self._open_library_dir)
        for w in (refresh_btn, rollback_btn, rename_btn, delete_btn, open_btn):
            button_row.addWidget(w)
        button_row.addStretch(1)
        layout.addLayout(button_row)
        return box

    # -- firmware version library ------------------------------------------

    def _refresh_library(self):
        if self.firmware_store is None:
            return
        try:
            rows = self.firmware_store.list(vehicle=self.vehicle_id)
        except Exception:  # noqa: BLE001 - library is best-effort
            return
        self._library_rows = rows
        table = self.library_table
        table.setRowCount(len(rows))
        for row, item in enumerate(rows):
            created = time.strftime("%Y-%m-%d %H:%M", time.localtime(item["created_at"]))
            size = item["size"]
            size_text = f"{size // 1024}KB" if size >= 1024 else f"{size}B"
            values = (
                created,
                item["note"],
                size_text,
                RESULT_TEXT.get(item["result"], item["result"]),
                Path(item["source_path"]).name,
            )
            for column, text in enumerate(values):
                table.setItem(row, column, QTableWidgetItem(str(text)))

    def _selected_library_row(self):
        row = self.library_table.currentRow()
        if 0 <= row < len(self._library_rows):
            return self._library_rows[row]
        return None

    def _rollback_selected(self):
        item = self._selected_library_row()
        if item is None:
            QMessageBox.information(self, "固件版本库", "请先在列表中选择一个版本。")
            return
        snapshot = Path(item["snapshot_path"])
        if not snapshot.is_file():
            QMessageBox.warning(self, "固件版本库", "快照文件已丢失，无法回退。")
            return
        if item["family"] in FLASH_GUIDANCE:
            self.family_combo.setCurrentText(item["family"])
        self.firmware_path.setText(str(snapshot))
        self.note_edit.setText(f"回退：{item['note']}")

    def _rename_selected(self):
        item = self._selected_library_row()
        if item is None or self.firmware_store is None:
            QMessageBox.information(self, "固件版本库", "请先在列表中选择一个版本。")
            return
        text, ok = QInputDialog.getText(self, "修改备注", "备注：", text=item["note"])
        if not ok:
            return
        try:
            self.firmware_store.update_note(item["id"], text.strip())
        except Exception as exc:  # noqa: BLE001
            QMessageBox.warning(self, "固件版本库", f"修改失败：{exc}")
        self._refresh_library()

    def _delete_selected(self):
        item = self._selected_library_row()
        if item is None or self.firmware_store is None:
            QMessageBox.information(self, "固件版本库", "请先在列表中选择一个版本。")
            return
        answer = QMessageBox.question(
            self, "固件版本库",
            f"确定删除「{item['note']}」这条版本记录吗？\n（无其他记录引用时快照文件一并删除）",
        )
        if answer != QMessageBox.StandardButton.Yes:
            return
        try:
            self.firmware_store.delete(item["id"])
        except Exception as exc:  # noqa: BLE001
            QMessageBox.warning(self, "固件版本库", f"删除失败：{exc}")
        self._refresh_library()

    def _open_library_dir(self):
        if self.firmware_store is None:
            return
        QDesktopServices.openUrl(QUrl.fromLocalFile(str(self.firmware_store.library_dir)))

    def _record_version(self, family, firmware):
        """Snapshot the image into the library; failure never blocks flashing."""
        self._active_version_id = None
        if self.firmware_store is None:
            return
        try:
            note = self.note_edit.text().strip()
            if not note:
                note = f"{Path(firmware).stem} @ {time.strftime('%m-%d %H:%M')}"
            self._active_version_id = self.firmware_store.record(
                self.vehicle_id, family, firmware, note
            )
        except Exception as exc:  # noqa: BLE001
            self.log.appendPlainText(f"固件版本记录失败（不影响烧录）：{exc}")
            self._active_version_id = None

    def _finish_version_record(self, result):
        if self._active_version_id is None or self.firmware_store is None:
            return
        try:
            self.firmware_store.set_result(self._active_version_id, result)
        except Exception:  # noqa: BLE001
            pass
        self._active_version_id = None
        self._refresh_library()

    # -- progress bar helpers -----------------------------------------------

    def _progress_reset(self):
        self.progress_bar.setRange(0, 100)
        self.progress_bar.setValue(0)
        self.stage_label.setText("就绪")

    def _progress_busy(self, text):
        if self.progress_bar.maximum() == 0:
            self.progress_bar.setValue(0)
        else:
            self.progress_bar.setRange(0, 0)
        self.stage_label.setText(text)

    def _progress_value(self, percent):
        if self.progress_bar.maximum() == 0:
            self.progress_bar.setRange(0, 100)
        self.progress_bar.setValue(int(round(percent)))
        self.stage_label.setText(f"写入中 {percent:.0f}%")

    # -- flashing ------------------------------------------------------------

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
        # TI ROM BSL is fixed at 9600 8N1; lock the selectors for MSPM0.
        self.baud_combo.setCurrentText("9600")
        self.baud_combo.setEnabled(not is_mspm0)
        self.mode_combo.setEnabled(not is_mspm0)
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

    def _serial_mode(self):
        return str(self.mode_combo.currentData() or DEFAULT_SERIAL_MODE)

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
        self._cancel_requested = False
        self._stm_pending = ""
        port = self.port_edit.text().strip()
        if not port:
            self._reject("未填写串口端口")
            return
        firmware = self.firmware_path.text().strip()
        if not firmware or not Path(firmware).is_file():
            self._reject("固件文件不存在，请重新选择")
            return
        family = self.family_combo.currentText()
        size_error = check_firmware_size(family, Path(firmware).stat().st_size)
        if size_error:
            self._reject(size_error)
            return
        self.state = self.state.transition(FlashState.VALIDATING, "校验烧录条件…")
        self._set_reason("校验烧录条件…")
        if self.transport is not None and self.transport.connected:
            self.transport.disconnect()
            self.log.appendPlainText("已自动断开车辆连接，释放串口。")
        self.settings.setValue("flash/port", port)
        self.settings.setValue("flash/baud", self.baud_combo.currentText())
        self._record_version(family, firmware)
        if family == MSPM0_FAMILY:
            self._start_mspm0_worker(port, firmware)
            return
        self.state = self.state.transition(FlashState.FLASHING, "正在烧录…")
        self._set_reason("正在烧录…")
        self._progress_busy("握手 bootloader…")
        command = build_flash_command(
            self.flash_backend, port, int(self.baud_combo.currentText()),
            firmware, serial_mode=self._serial_mode(),
        )
        self.log.appendPlainText("$ " + " ".join(command))
        self.process = QProcess(self)
        self.process.readyReadStandardOutput.connect(self._on_output)
        self.process.readyReadStandardError.connect(self._on_error_output)
        self.process.finished.connect(self._on_finished)
        self.process.start(command[0], command[1:])

    def _start_mspm0_worker(self, port, firmware):
        self.state = self.state.transition(FlashState.FLASHING, "正在烧录…")
        self._set_reason("正在烧录…")
        self.log.appendPlainText(f"以 9600-8N1 连接 {port}，使用内置 TI ROM BSL 驱动。")
        self.worker = Mspm0FlashWorker(port, firmware, self)
        self.worker.log_line.connect(self.log.appendPlainText)
        self.worker.progress.connect(self._on_mspm0_progress)
        self.worker.stage.connect(self._on_mspm0_stage)
        self.worker.finished_with_code.connect(self._on_mspm0_finished)
        self.worker.start()

    def _on_mspm0_stage(self, name):
        self._progress_busy(MSPM0_STAGE_TEXTS.get(name, name))

    def _on_mspm0_progress(self, written, total):
        if total > 0:
            self._progress_value(100.0 * written / total)

    def _cancel_flash(self):
        if self.state.state != FlashState.FLASHING:
            return
        self._cancel_requested = True
        if self.worker is not None:
            self.worker.cancel()
            self._set_reason("正在取消…")
        elif self.process is not None:
            self.process.kill()
            self._set_reason("正在取消…")

    # -- MSPM0 worker results -------------------------------------------------

    def _on_mspm0_finished(self, code):
        worker, self.worker = self.worker, None
        cancelled = worker is not None and worker.was_cancelled
        if code == 0:
            self.state = self.state.transition(FlashState.VERIFYING, "写入完成，回读校验通过")
            self.state = self.state.transition(FlashState.SUCCEEDED, "烧录成功")
            self._set_reason("烧录成功", good=True)
            self.stage_label.setText("烧录成功")
            self.progress_bar.setRange(0, 100)
            self.progress_bar.setValue(100)
            self.log.appendPlainText("=== 烧录成功，应用已由 BSL 启动。 ===")
            self._finish_version_record(RESULT_OK)
        elif cancelled:
            self.state = self.state.transition(FlashState.CANCELLED, "已取消")
            self._set_reason("已取消")
            self.stage_label.setText("已取消")
            self._finish_version_record(RESULT_CANCELLED)
        else:
            self.state = self.state.transition(FlashState.FAILED, "烧录失败，详见日志")
            self._set_reason("烧录失败，详见日志")
            self.stage_label.setText("烧录失败")
            self.log.appendPlainText(
                "=== 烧录失败。确认车辆已进 BSL、蓝牙为 9600-8N1、PA10/PA11 接线正确后重试。 ==="
            )
            self._finish_version_record(RESULT_FAILED)
        self.state = self.state.transition(FlashState.IDLE)
        self._refresh_action_buttons()

    # -- stm32flash subprocess -------------------------------------------------

    def _on_output(self):
        if self.process is None:
            return
        text = bytes(self.process.readAllStandardOutput()).decode(
            "utf-8", errors="replace"
        )
        self._stm_pending += text
        segments = split_output_segments(self._stm_pending)
        # 末段可能被串口分块截断（\r 前缀的进度行没有换行符），留到下个数据块。
        self._stm_pending = segments.pop()
        for segment in segments:
            self._handle_stm_segment(segment)

    def _handle_stm_segment(self, segment):
        kind, percent = classify_output_segment(segment)
        if kind == "progress":
            self._progress_value(percent)
            return
        text = segment.strip()
        if not text:
            return
        self.log.appendPlainText(text)
        if "Erasing memory" in text:
            self._progress_busy("擦除中…")
        elif "Write to memory" in text:
            self._progress_busy("写入中…")

    def _on_error_output(self):
        if self.process is None:
            return
        text = bytes(self.process.readAllStandardError()).decode(
            "utf-8", errors="replace"
        )
        for line in text.splitlines():
            line = line.strip()
            if line:
                self.log.appendPlainText("[stm32flash] " + line)

    def _on_finished(self, code, _status):
        self.process = None
        if self._stm_pending:
            self._handle_stm_segment(self._stm_pending)
            self._stm_pending = ""
        if code == 0:
            self.state = self.state.transition(FlashState.VERIFYING, "写入完成，回读校验通过")
            self.state = self.state.transition(FlashState.SUCCEEDED, "烧录成功")
            self._set_reason("烧录成功", good=True)
            self.progress_bar.setRange(0, 100)
            self.progress_bar.setValue(100)
            self.stage_label.setText("烧录成功")
            self.log.appendPlainText("=== 烧录成功。请断电将 BOOT0 挪回 0 后重启车辆。 ===")
            self._finish_version_record(RESULT_OK)
        elif self._cancel_requested:
            self.state = self.state.transition(FlashState.CANCELLED, "已取消")
            self._set_reason("已取消")
            self.stage_label.setText("已取消")
            self._finish_version_record(RESULT_CANCELLED)
        else:
            self.state = self.state.transition(FlashState.FAILED, f"烧录失败（退出码 {code}），详见日志")
            self._set_reason(f"烧录失败（退出码 {code}），详见日志")
            self.stage_label.setText("烧录失败")
            self.log.appendPlainText("=== 烧录失败。检查 BOOT0 是否在 1、串口是否被占用后重试。 ===")
            self._finish_version_record(RESULT_FAILED)
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
