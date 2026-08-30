"""A TI MSPM0 ROM BSL device simulator for hardware-free driver tests.

Implements the SLAU887 wire behaviour the driver expects: request packets
(0x80 | len | payload | crc32), transport-ack byte, core response packets
(0x08 | len | payload | crc32), flash memory image, password locking, and
absolute-address erases/programming with 8-byte padding semantics.
"""

import time

from core.mspm0_bsl import (
    CMD_CONNECTION,
    CMD_ERASE_RANGE,
    CMD_GET_IDENTITY,
    CMD_PROGRAM_DATA,
    CMD_START_APPLICATION,
    CMD_UNLOCK,
    CMD_VERIFY_CRC,
    CoreStatus,
    mspm0_crc32,
)

REQUEST_HEADER = 0x80
RESPONSE_HEADER = 0x08

FLASH_BASE = 0x41C00000
FLASH_SIZE = 128 * 1024


class FakeBslDevice:
    def __init__(self, password= b"\xff" * 32, max_buffer_size=144):
        self.password = password
        self.max_buffer_size = max_buffer_size
        self.memory = bytearray(b"\xff" * FLASH_SIZE)
        self.connected = False
        self.unlocked = False
        self.started = False
        self.erased_ranges = []
        self.next_transport_ack = 0x00
        self.next_status = CoreStatus.SUCCESS
        self.corrupt_next_response_crc = False
        self.closed = False

    # -- serial-like transport the driver talks to -------------------------
    def write(self, data: bytes) -> int:
        if self.closed:
            raise OSError("device closed")
        self._inbound = getattr(self, "_inbound", b"") + bytes(data)
        self._process()
        return len(data)

    def flush(self) -> None:
        pass

    def read(self, count: int) -> bytes:
        if self.closed:
            raise OSError("device closed")
        out = getattr(self, "_outbound", b"")
        chunk, self._outbound = out[:count], out[count:]
        return chunk

    # -- device side --------------------------------------------------------
    def _send(self, payload: bytes) -> None:
        packet = bytearray([RESPONSE_HEADER])
        packet += len(payload).to_bytes(2, "little")
        packet += payload
        crc = mspm0_crc32(payload)
        if self.corrupt_next_response_crc:
            crc ^= 0xFFFFFFFF
            self.corrupt_next_response_crc = False
        packet += crc.to_bytes(4, "little")
        self._outbound = getattr(self, "_outbound", b"") + bytes(packet)

    def _ack_and_status(self, status: CoreStatus = None) -> None:
        self._outbound = getattr(self, "_outbound", b"") + bytes([self.next_transport_ack])
        self.next_transport_ack = 0x00
        status = self.next_status if status is None else status
        self._send(bytes([0x3B, status if isinstance(status, int) else status.value]))
        self.next_status = CoreStatus.SUCCESS

    def _process(self) -> None:
        inbound = getattr(self, "_inbound", b"")
        while True:
            if len(inbound) < 3:
                self._inbound = inbound
                return
            payload_len = int.from_bytes(inbound[1:3], "little")
            total = 3 + payload_len + 4
            if len(inbound) < total:
                self._inbound = inbound
                return
            packet, inbound = inbound[:total], inbound[total:]
            payload = packet[3:3 + payload_len]
            expected_crc = int.from_bytes(packet[3 + payload_len:], "little")
            if packet[0] != REQUEST_HEADER or mspm0_crc32(payload) != expected_crc:
                self._outbound += bytes([0x52])
                continue
            self._handle(payload)

    def _handle(self, payload: bytes) -> None:
        command = payload[0]
        if command == CMD_CONNECTION:
            # Per SLAU887 the connection handshake answers with the transport
            # ack only; there is no core status response.
            self.connected = True
            self.unlocked = False
            self._outbound += bytes([self.next_transport_ack])
            self.next_transport_ack = 0x00
        elif command == CMD_GET_IDENTITY:
            info = bytes([
                0x31,
                0x06, 0x00,  # command interpreter version 6
                0x01, 0x00,  # build id
                0, 0, 0, 0,  # application revision
                0x00, 0x00,  # plugin version
            ]) + self.max_buffer_size.to_bytes(2, "little") + bytes([
                0x00, 0x20, 0x20, 0x02,  # buffer start address
                0, 0, 0, 0,  # bcr config id
                0, 0, 0, 0,  # bsl config id
            ])
            self._outbound += bytes([self.next_transport_ack])
            self.next_transport_ack = 0x00
            self._send(info)
        elif command == CMD_UNLOCK:
            supplied = payload[1:33]
            if supplied != self.password:
                self._ack_and_status(CoreStatus.PASSWORD_ERROR)
            else:
                self.unlocked = True
                self._ack_and_status()
        elif command == CMD_ERASE_RANGE:
            start = int.from_bytes(payload[1:5], "little")
            end = int.from_bytes(payload[5:9], "little")
            offset = start - FLASH_BASE
            size = end - start
            if offset < 0 or offset + size > FLASH_SIZE:
                self._ack_and_status(CoreStatus.INVALID_MEMORY_RANGE)
                return
            self.memory[offset:offset + size] = b"\xff" * size
            self.erased_ranges.append((start, end))
            self._ack_and_status()
        elif command == CMD_PROGRAM_DATA:
            address = int.from_bytes(payload[1:5], "little")
            data = payload[5:]
            if not self.unlocked:
                self._ack_and_status(CoreStatus.LOCKED)
                return
            offset = address - FLASH_BASE
            if offset < 0 or offset + len(data) > FLASH_SIZE:
                self._ack_and_status(CoreStatus.INVALID_MEMORY_RANGE)
                return
            self.memory[offset:offset + len(data)] = data
            self._ack_and_status()
        elif command == CMD_VERIFY_CRC:
            start = int.from_bytes(payload[1:5], "little")
            length = int.from_bytes(payload[5:9], "little")
            offset = start - FLASH_BASE
            data = bytes(self.memory[offset:offset + length])
            self._outbound += bytes([self.next_transport_ack])
            self.next_transport_ack = 0x00
            self._send(bytes([0x32]) + mspm0_crc32(data).to_bytes(4, "little"))
        elif command == CMD_START_APPLICATION:
            # The device leaves the BSL right after the transport ack.
            self.started = True
            self.connected = False
            self.unlocked = False
            self._outbound += bytes([self.next_transport_ack])
            self.next_transport_ack = 0x00
        else:
            self._ack_and_status(CoreStatus.UNKNOWN_COMMAND)


class FakeSerial:
    """Bidirectional pipe: driver <-> FakeBslDevice, with canned wire faults."""

    def __init__(self, device: FakeBslDevice):
        self.device = device
        device._outbound = b""
        self._pending = b""

    def write(self, data: bytes) -> int:
        self.device.write(data)
        return len(data)

    def flush(self) -> None:
        pass

    def read(self, count: int) -> bytes:
        # Simulator answers synchronously during write(), so wait briefly for
        # the device to produce bytes; raising only on a closed device.
        deadline = time.monotonic() + 5.0
        while len(self._pending) < count:
            chunk = self.device.read(4096)
            self._pending += chunk
            if self.device.closed:
                raise OSError("device closed")
            if len(self._pending) < count and time.monotonic() >= deadline:
                raise TimeoutError("simulator produced no data")
            if len(self._pending) < count:
                time.sleep(0.005)
        out, self._pending = self._pending[:count], self._pending[count:]
        return out

    def close(self) -> None:
        self.device.closed = True
