import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))
sys.path.insert(0, str(ROOT / "tests"))

from core.mspm0_bsl import (
    MSPM0G_FLASH_BASE,
    MSPM0G_FLASH_END,
    BslCancelled,
    BslError,
    CoreStatus,
    Mspm0RomBsl,
    encode_command,
    flash_image,
    mspm0_crc32,
    decode_core_response,
)
from bsl_simulator import FLASH_BASE, FLASH_SIZE, FakeBslDevice, FakeSerial

IMAGE = bytes(range(256)) * 64  # 16 KiB pseudo firmware


class Mspm0CrcTests(unittest.TestCase):
    def test_crc_matches_ported_rust_variant_without_final_xor(self):
        # The Rust driver computed the standard reflected CRC-32 but skipped
        # the final inversion; zlib applies it, so undo it.
        self.assertEqual(0x340BC6D9, mspm0_crc32(b"123456789"))

    def test_encode_command_frames_payload_with_crc(self):
        packet = encode_command(0x12)
        self.assertEqual(0x80, packet[0])
        self.assertEqual((1).to_bytes(2, "little"), packet[1:3])
        self.assertEqual(bytes([0x12]), packet[3:4])
        self.assertEqual(mspm0_crc32(b"\x12").to_bytes(4, "little"), packet[4:])


class Mspm0FlashFlowTests(unittest.TestCase):
    def setUp(self):
        self.device = FakeBslDevice()
        self.transport = FakeSerial(self.device)

    def test_full_flash_recipe_programs_device_memory(self):
        log = []
        stages = []
        flash_image(self.transport, IMAGE, log=log.append, stage=stages.append)
        self.assertTrue(self.device.started)
        self.assertEqual(
            IMAGE,
            bytes(self.device.memory[: len(IMAGE)]),
            "写入后设备存储应与固件一致",
        )
        # 擦除覆盖整个用户区：新固件比旧固件短时也不会残留旧代码
        self.assertEqual(
            [(MSPM0G_FLASH_BASE, MSPM0G_FLASH_END)],
            self.device.erased_ranges,
        )
        self.assertIn("BSL 解锁成功", log)
        self.assertIn("CRC 校验通过，启动应用…", log)
        self.assertEqual(
            ["connecting", "erasing", "programming", "verifying", "starting"],
            stages,
        )

    def test_erase_can_be_limited_to_image_range(self):
        flash_image(self.transport, IMAGE, erase_full_user_area=False)
        self.assertEqual(
            [(MSPM0G_FLASH_BASE, MSPM0G_FLASH_BASE + len(IMAGE))],
            self.device.erased_ranges,
        )

    def test_wrong_password_fails_with_password_error(self):
        with self.assertRaises(BslError) as ctx:
            flash_image(self.transport, IMAGE, password=b"\x00" * 32)
        self.assertEqual("core", ctx.exception.kind)
        self.assertIn("密码错误", ctx.exception.detail)

    def test_verify_catches_device_memory_corruption(self):
        # Simulate a flipped bit on the device between program and verify.
        driver = Mspm0RomBsl(self.transport)
        driver.connect()
        driver.device_info()
        driver.unlock()
        driver.erase_range(FLASH_BASE, FLASH_BASE + len(IMAGE))
        driver.program(FLASH_BASE, IMAGE)
        self.device.memory[10] ^= 0xFF
        with self.assertRaises(BslError) as ctx:
            driver.verify_crc(FLASH_BASE, len(IMAGE), mspm0_crc32(IMAGE))
        self.assertEqual("verify_mismatch", ctx.exception.kind)

    def test_transport_ack_error_surfaces_detail(self):
        self.device.next_transport_ack = 0x52
        with self.assertRaises(BslError) as ctx:
            flash_image(self.transport, IMAGE)
        self.assertEqual("transport_ack", ctx.exception.kind)
        self.assertIn("校验和错误", ctx.exception.detail)

    def test_response_crc_corruption_is_rejected(self):
        self.device.corrupt_next_response_crc = True
        with self.assertRaises(BslError) as ctx:
            flash_image(self.transport, IMAGE)
        self.assertIn(ctx.exception.kind, ("response_crc", "unknown_response"))

    def test_device_memory_range_error_is_reported(self):
        with self.assertRaises(BslError) as ctx:
            flash_image(self.transport, IMAGE, base_address=0x00000000)
        self.assertEqual("core", ctx.exception.kind)
        self.assertIn("地址范围无效", ctx.exception.detail)

    def test_cancellation_stops_programming_midway(self):
        seen = []

        def progress(written, total):
            seen.append(written)

        def stop_after_half():
            return not seen or seen[-1] < len(IMAGE) // 2

        with self.assertRaises(BslCancelled):
            flash_image(
                self.transport, IMAGE,
                should_continue=stop_after_half, progress=progress,
            )
        self.assertTrue(seen)
        self.assertFalse(self.device.started)

    def test_cancellation_responds_during_blocking_wait(self):
        # 设备沉默（模拟芯片在擦除/响应前不回字节）时，取消也应立即生效，
        # 而不是等满 15s 读超时。
        class SilentTransport:
            def __init__(self, device):
                self.device = device
                self.writes = 0

            def write(self, data):
                self.writes += 1
                return self.device.write(data)

            def flush(self):
                pass

            def read(self, count):
                return b""  # 永远沉默

        transport = SilentTransport(self.device)
        driver = Mspm0RomBsl(transport, timeout_s=30.0, should_continue=lambda: False)
        with self.assertRaises(BslCancelled):
            driver.connect()
        self.assertEqual(1, transport.writes)

    def test_identity_response_is_parsed_for_logging(self):
        driver = Mspm0RomBsl(self.transport)
        driver.connect()
        info = driver.device_info()
        self.assertEqual(6, info.command_interpreter_version)
        self.assertEqual(144, info.max_buffer_size)

    def test_program_chunks_pad_to_eight_byte_alignment(self):
        odd_image = b"\x01\x02\x03"  # 3 bytes -> one 8-byte padded packet
        flash_image(self.transport, odd_image)
        self.assertEqual(odd_image, bytes(self.device.memory[:3]))
        self.assertEqual(b"\xff" * 5, bytes(self.device.memory[3:8]))

    def test_locked_device_rejects_programming(self):
        driver = Mspm0RomBsl(self.transport)
        driver.connect()
        driver.device_info()
        with self.assertRaises(BslError) as ctx:
            driver.program(FLASH_BASE, IMAGE)
        self.assertEqual("state", ctx.exception.kind)

    def test_disconnected_device_maps_to_disconnected(self):
        self.transport.device.closed = True
        with self.assertRaises(BslError) as ctx:
            flash_image(self.transport, IMAGE)
        self.assertEqual("disconnected", ctx.exception.kind)


if __name__ == "__main__":
    unittest.main()
