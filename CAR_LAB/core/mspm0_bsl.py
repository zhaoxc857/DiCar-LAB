"""TI MSPM0 ROM BSL (UART) driver, ported from the pre-migration Rust
implementation (crates/dicar-firmware-flash/src/bsl.rs, commit 56aa975).

Protocol reference: TI SLAU887 (MSPM0 Bootloader User's Guide). The wire
format is one 9600-8N1 UART: request packets start with 0x80 and carry a
u16 LE payload length, the command payload, and a CRC-32 of the payload;
the device answers with a single transport-ack byte followed by a 0x08
core response packet for commands that return data or status.

Known deviations from the Rust original, all flagged for the first on-car
verification because the original was never hardware-tested:

- Addresses here are ABSOLUTE device addresses. The Rust code erased and
  programmed from offset 0, which the MSPM0 BSL would reject as
  InvalidMemoryRange; real MSPM0G main flash starts at 0x41C00000.
- The Rust CRC-32 loop omits the final XOR of the standard CRC-32/ISO-HDLC
  variant. This port reproduces that choice bit-for-bit (zlib.crc32 with
  the final inversion undone); if the first hardware handshake fails with
  transport ack 0x52 (checksum incorrect), this is the first suspect.
- The default BSL password (32 x 0xFF) applies when NONMAIN still ships
  factory settings; a custom password needs NONMAIN programming.
"""

from __future__ import annotations

import time
import zlib
from enum import IntEnum
from pathlib import Path

REQUEST_HEADER = 0x80
RESPONSE_HEADER = 0x08
MAX_CORE_RESPONSE_PAYLOAD = 1024
MAX_PROGRAM_DATA = 128
PROGRAM_ALIGNMENT = 8

MSPM0G_FLASH_BASE = 0x41C00000
DEFAULT_BSL_PASSWORD = b"\xff" * 32
G3507_MAIN_FLASH_SIZE = 128 * 1024

CMD_CONNECTION = 0x12
CMD_GET_IDENTITY = 0x19
CMD_UNLOCK = 0x21
CMD_ERASE_RANGE = 0x23
CMD_PROGRAM_DATA = 0x20
CMD_VERIFY_CRC = 0x26
CMD_START_APPLICATION = 0x40


class CoreStatus(IntEnum):
    SUCCESS = 0x00
    LOCKED = 0x01
    PASSWORD_ERROR = 0x02
    MULTIPLE_PASSWORD_ERROR = 0x03
    UNKNOWN_COMMAND = 0x04
    INVALID_MEMORY_RANGE = 0x05
    INVALID_COMMAND = 0x06
    FACTORY_RESET_DISABLED = 0x07
    FACTORY_RESET_PASSWORD_ERROR = 0x08
    READOUT_DISABLED = 0x09
    INVALID_ADDRESS_LENGTH_ALIGNMENT = 0x0A
    VERIFICATION_INVALID_LENGTH = 0x0B
    FLASH_PROGRAM_FAILED = 0xF1
    MASS_ERASE_FAILED = 0xF2
    FLASH_ERASE_FAILED = 0xF3
    FACTORY_RESET_FAILED = 0xF4

    @classmethod
    def from_byte(cls, value: int) -> "CoreStatus":
        try:
            return cls(value)
        except ValueError:
            return cls(0x04)


STATUS_MESSAGES = {
    CoreStatus.SUCCESS: "成功",
    CoreStatus.LOCKED: "BSL 处于锁定状态",
    CoreStatus.PASSWORD_ERROR: "BSL 密码错误",
    CoreStatus.MULTIPLE_PASSWORD_ERROR: "多次密码错误，需要复位后重试",
    CoreStatus.UNKNOWN_COMMAND: "未知命令",
    CoreStatus.INVALID_MEMORY_RANGE: "地址范围无效",
    CoreStatus.INVALID_COMMAND: "命令无效",
    CoreStatus.FACTORY_RESET_DISABLED: "工厂复位被禁用",
    CoreStatus.FACTORY_RESET_PASSWORD_ERROR: "工厂复位密码错误",
    CoreStatus.READOUT_DISABLED: "回读被禁用",
    CoreStatus.INVALID_ADDRESS_LENGTH_ALIGNMENT: "地址/长度未按对齐要求",
    CoreStatus.VERIFICATION_INVALID_LENGTH: "校验长度无效",
    CoreStatus.FLASH_PROGRAM_FAILED: "Flash 写入失败",
    CoreStatus.MASS_ERASE_FAILED: "整片擦除失败",
    CoreStatus.FLASH_ERASE_FAILED: "扇区擦除失败",
    CoreStatus.FACTORY_RESET_FAILED: "工厂复位失败",
}

TRANSPORT_ACK_MESSAGES = {
    0x51: "包头错误",
    0x52: "校验和错误",
    0x53: "包长度为零",
    0x54: "包长度超限",
    0x55: "未知错误",
    0x56: "未知波特率",
    0x57: "包大小不支持",
}


class BslError(Exception):
    """Transport/protocol failure. `kind` is a stable machine-readable tag."""

    def __init__(self, kind: str, detail: str = ""):
        super().__init__(f"{kind}: {detail}" if detail else kind)
        self.kind = kind
        self.detail = detail


class BslCancelled(BslError):
    def __init__(self):
        super().__init__("cancelled", "用户取消了烧录")


def mspm0_crc32(data: bytes) -> int:
    """CRC-32 as implemented by the pre-migration driver (no final XOR).

    zlib.crc32 already applies the final XOR of CRC-32/ISO-HDLC, so undo it
    to reproduce the Rust wire format bit-for-bit.
    """
    return zlib.crc32(data) ^ 0xFFFFFFFF


class DeviceInfo:
    def __init__(self, payload: bytes):
        (
            self.command_interpreter_version,
            self.build_id,
            self.application_revision,
            self.plugin_version,
            self.max_buffer_size,
            self.buffer_start_address,
            self.bcr_config_id,
            self.bsl_config_id,
        ) = _unpack_le(payload, [2, 2, 4, 2, 2, 4, 4, 4])


def _unpack_le(payload: bytes, sizes: list) -> tuple:
    values = []
    offset = 0
    for size in sizes:
        values.append(int.from_bytes(payload[offset:offset + size], "little"))
        offset += size
    return tuple(values)


def encode_command(command: int, payload_tail: bytes = b"") -> bytes:
    payload = bytes([command]) + payload_tail
    packet = bytearray()
    packet.append(REQUEST_HEADER)
    packet += len(payload).to_bytes(2, "little")
    packet += payload
    packet += mspm0_crc32(payload).to_bytes(4, "little")
    return bytes(packet)


def parse_transport_ack(byte: int) -> None:
    if byte == 0x00:
        return
    message = TRANSPORT_ACK_MESSAGES.get(byte, f"未知传输错误 0x{byte:02X}")
    raise BslError("transport_ack", message)


class CoreResponse:
    def __init__(self, kind: str, status: "CoreStatus | None" = None,
                 crc: int | None = None, identity: "DeviceInfo | None" = None):
        self.kind = kind
        self.status = status
        self.crc = crc
        self.identity = identity


def decode_core_response(packet: bytes) -> CoreResponse:
    if len(packet) < 8:
        raise BslError("response_length", "响应包过短")
    if packet[0] != RESPONSE_HEADER:
        raise BslError("response_header", f"响应包头 0x{packet[0]:02X} 错误")
    payload_len = int.from_bytes(packet[1:3], "little")
    if len(packet) != 3 + payload_len + 4:
        raise BslError("response_length", "响应包长度不匹配")
    payload = packet[3:3 + payload_len]
    expected_crc = int.from_bytes(packet[3 + payload_len:], "little")
    if mspm0_crc32(payload) != expected_crc:
        raise BslError("response_crc", "响应包 CRC 校验失败")
    if payload[:1] == b"\x3b" and len(payload) == 2:
        return CoreResponse("status", status=CoreStatus.from_byte(payload[1]))
    if payload[:1] == b"\x32" and len(payload) == 5:
        return CoreResponse("crc", crc=int.from_bytes(payload[1:], "little"))
    if payload[:1] == b"\x31" and len(payload) == 25:
        return CoreResponse("identity", identity=DeviceInfo(payload[1:]))
    raise BslError("unknown_response", f"无法识别的响应 0x{payload[:1].hex()}")


class Mspm0RomBsl:
    """Driver over a blocking 9600-8N1 byte transport (pyserial or test fake).

    The transport must provide write(bytes)->int, flush(), and read(n)->bytes
    where read returns up to n bytes and b"" means the link went away.
    """

    def __init__(self, transport, timeout_s: float = 15.0):
        self.transport = transport
        self.timeout_s = timeout_s
        self.connected = False
        self.unlocked = False
        self.info: DeviceInfo | None = None

    def connect(self) -> None:
        self._write_and_ack(CMD_CONNECTION)
        self.connected = True
        self.unlocked = False
        self.info = None

    def device_info(self) -> DeviceInfo:
        self._require_connected()
        self._write_and_ack(CMD_GET_IDENTITY)
        response = self._read_core_response()
        if response.kind != "identity":
            raise BslError("unknown_response", "期待设备身份响应")
        if response.identity.max_buffer_size <= 5:
            raise BslError("invalid_buffer_size", "设备缓冲区过小")
        self.info = response.identity
        return response.identity

    def unlock(self, password: bytes = DEFAULT_BSL_PASSWORD) -> None:
        self._require_connected()
        if len(password) != 32:
            raise BslError("state", "BSL 密码必须为 32 字节")
        self._write_and_ack(CMD_UNLOCK, password)
        self._expect_success_status()
        self.unlocked = True

    def erase_range(self, start: int, end: int) -> None:
        self._require_unlocked()
        if start > end:
            raise BslError("state", "擦除起点大于终点")
        self._write_and_ack(CMD_ERASE_RANGE, start.to_bytes(4, "little") + end.to_bytes(4, "little"))
        self._expect_success_status()

    def program(self, address: int, image: bytes,
                should_continue=None, progress=None) -> int:
        self._require_unlocked()
        if address % PROGRAM_ALIGNMENT:
            raise BslError("state", "写入地址未按 8 字节对齐")
        info = self.info
        if info is None:
            raise BslError("state", "尚未读取设备信息")
        available = max(0, info.max_buffer_size - 5)
        chunk_size = min(available, MAX_PROGRAM_DATA) // PROGRAM_ALIGNMENT * PROGRAM_ALIGNMENT
        if chunk_size < PROGRAM_ALIGNMENT:
            raise BslError("invalid_buffer_size", "有效写入块过小")
        offset = 0
        while offset < len(image):
            if should_continue is not None and not should_continue():
                raise BslCancelled()
            actual_len = min(len(image) - offset, chunk_size)
            padded_len = (actual_len + PROGRAM_ALIGNMENT - 1) // PROGRAM_ALIGNMENT * PROGRAM_ALIGNMENT
            padded = image[offset:offset + actual_len] + b"\xff" * (padded_len - actual_len)
            self._write_and_ack(CMD_PROGRAM_DATA, (address + offset).to_bytes(4, "little") + padded)
            self._expect_success_status()
            offset += actual_len
            if progress is not None:
                progress(offset, len(image))
        return len(image)

    def verify_crc(self, address: int, length: int, expected: int) -> None:
        self._require_unlocked()
        self._write_and_ack(CMD_VERIFY_CRC, address.to_bytes(4, "little") + length.to_bytes(4, "little"))
        response = self._read_core_response()
        if response.kind == "crc":
            if response.crc != expected:
                raise BslError(
                    "verify_mismatch",
                    f"期望 0x{expected:08X}，实际 0x{response.crc:08X}",
                )
            return
        if response.kind == "status" and response.status != CoreStatus.SUCCESS:
            raise BslError("core", STATUS_MESSAGES.get(response.status, response.status.name))
        raise BslError("unknown_response", "期待 CRC 校验响应")

    def start_application(self) -> None:
        self._require_connected()
        self._write_and_ack(CMD_START_APPLICATION)
        self.connected = False
        self.unlocked = False

    def _write_and_ack(self, command: int, payload_tail: bytes = b"") -> None:
        packet = encode_command(command, payload_tail)
        try:
            self.transport.write(packet)
            self.transport.flush()
            ack = self._read_exact(1)
        except BslError:
            raise
        except OSError as exc:
            raise BslError("disconnected", str(exc)) from exc
        parse_transport_ack(ack[0])

    def _read_core_response(self) -> CoreResponse:
        header = self._read_exact(3)
        payload_len = int.from_bytes(header[1:3], "little")
        if payload_len == 0 or payload_len > MAX_CORE_RESPONSE_PAYLOAD:
            raise BslError("response_length", f"响应长度 {payload_len} 越界")
        rest = self._read_exact(payload_len + 4)
        return decode_core_response(header + rest)

    def _read_exact(self, count: int) -> bytes:
        deadline = time.monotonic() + self.timeout_s
        buffer = bytearray()
        while len(buffer) < count:
            chunk = self.transport.read(count - len(buffer))
            if not chunk:
                if time.monotonic() >= deadline:
                    raise BslError("timeout", f"等待响应超时（已收 {len(buffer)}/{count} 字节）")
                continue
            buffer += chunk
        return bytes(buffer)

    def _expect_success_status(self) -> None:
        response = self._read_core_response()
        if response.kind == "status":
            if response.status == CoreStatus.SUCCESS:
                return
            raise BslError("core", STATUS_MESSAGES.get(response.status, response.status.name))
        raise BslError("unknown_response", "期待状态响应")

    def _require_connected(self) -> None:
        if not self.connected:
            raise BslError("state", "尚未与 BSL 建立连接")

    def _require_unlocked(self) -> None:
        if not (self.connected and self.unlocked):
            raise BslError("state", "BSL 尚未解锁")


def flash_image(transport, image: bytes, base_address: int = MSPM0G_FLASH_BASE,
                password: bytes = DEFAULT_BSL_PASSWORD,
                should_continue=None, progress=None, log=None) -> DeviceInfo:
    """Run the full validated recipe: connect, info, unlock, erase, program,
    verify, start. Raises BslError on any failure."""
    driver = Mspm0RomBsl(transport)

    def say(message: str):
        if log is not None:
            log(message)

    driver.connect()
    info = driver.device_info()
    say(f"BSL 连接成功：解释器 v{info.command_interpreter_version}，缓冲 {info.max_buffer_size} 字节")
    driver.unlock(password)
    say("BSL 解锁成功")
    driver.erase_range(base_address, base_address + len(image))
    say("擦除完成，开始写入…")
    driver.program(base_address, image, should_continue=should_continue, progress=progress)
    say("写入完成，回读 CRC 校验…")
    driver.verify_crc(base_address, len(image), mspm0_crc32(image))
    say("CRC 校验通过，启动应用…")
    driver.start_application()
    return info
