import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core.slcan import CanFrame, encode_frame, parse_line


class SlcanFrameTests(unittest.TestCase):
    def test_standard_frame_roundtrip(self):
        frame = CanFrame(0x123, b"\x01\x02\x03\x04")
        line = encode_frame(frame)
        self.assertEqual("t123401020304", line)
        parsed = parse_line(line + "\r")
        self.assertEqual(frame, parsed)

    def test_extended_and_remote_frames(self):
        ext = CanFrame(0x1ABCDEF0, b"\xAA\xBB", extended=True)
        self.assertEqual("T1ABCDEF02AABB", encode_frame(ext))
        self.assertEqual(ext, parse_line(encode_frame(ext) + "\r"))
        remote = CanFrame(0x456, remote=True)
        self.assertEqual("r4560", encode_frame(remote))
        self.assertEqual(remote, parse_line("r4560\r"))

    def test_non_frame_lines_return_none(self):
        for line in ("", "\r", "OK", "NOPE", "O", "C", "z1230", "t12388"):
            self.assertIsNone(parse_line(line),
                              f"{line!r} 不是合法帧，应返回 None")

    def test_dlc_mismatch_is_rejected(self):
        self.assertIsNone(parse_line("t12348AABB"))  # 声明 8 字节只有 2 字节


if __name__ == "__main__":
    unittest.main()
