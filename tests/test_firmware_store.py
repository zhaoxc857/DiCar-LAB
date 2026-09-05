import hashlib
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core.firmware_store import FirmwareStore


class FirmwareStoreTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        base = Path(self._tmp.name)
        self.store = FirmwareStore(base / "lib.db", base / "library")

    def tearDown(self):
        self._tmp.cleanup()

    def _fw(self, name, content):
        path = Path(self._tmp.name) / name
        path.write_bytes(content)
        return path

    def test_record_snapshots_and_dedupes_by_sha256(self):
        fw = self._fw("car.bin", b"\x01\x02\x03\x04")
        first = self.store.record("veh", "MSPM0G3507", str(fw), "v1 初版")
        second = self.store.record("veh", "MSPM0G3507", str(fw), "v1 重烧验证")
        self.assertEqual(1, len(list(self.store.library_dir.glob("*.bin"))),
                         "同一镜像多次烧录只保留一份快照")
        rows = self.store.list(vehicle="veh")
        self.assertEqual(2, len(rows))
        digest = hashlib.sha256(b"\x01\x02\x03\x04").hexdigest()
        for row in rows:
            self.assertEqual(digest, row["sha256"])
            self.assertEqual(len(fw.read_bytes()), row["size"])
            self.assertTrue(Path(row["snapshot_path"]).is_file(),
                            "快照必须存在且可直接作为回退烧录源")
            self.assertEqual(b"\x01\x02\x03\x04", Path(row["snapshot_path"]).read_bytes())
        self.assertEqual({first, second}, {row["id"] for row in rows})

    def test_result_and_note_updates(self):
        fw = self._fw("a.bin", b"A" * 16)
        version_id = self.store.record("veh", "STM32F1", str(fw), "")
        self.assertEqual("pending", self.store.get(version_id)["result"])
        self.store.set_result(version_id, "ok")
        self.store.update_note(version_id, "转向环 v2")
        row = self.store.get(version_id)
        self.assertEqual("ok", row["result"])
        self.assertEqual("转向环 v2", row["note"])

    def test_delete_removes_snapshot_only_when_unreferenced(self):
        fw = self._fw("b.bin", b"B" * 16)
        first = self.store.record("veh", "STM32F1", str(fw), "first")
        second = self.store.record("veh", "STM32F1", str(fw), "second")
        snapshots = list(self.store.library_dir.glob("*.bin"))
        self.assertEqual(1, len(snapshots))
        self.store.delete(first)
        self.assertIsNotNone(self.store.get(second))
        self.assertTrue(snapshots[0].exists(), "仍有记录引用时不删快照")
        self.store.delete(second)
        self.assertIsNone(self.store.get(second))
        self.assertFalse(snapshots[0].exists(), "无引用时快照一并删除")

    def test_list_filters_by_vehicle(self):
        fw = self._fw("c.bin", b"C" * 16)
        self.store.record("carA", "STM32F1", str(fw), "x")
        self.store.record("carB", "STM32F1", str(fw), "y")
        self.assertEqual(1, len(self.store.list(vehicle="carA")))
        self.assertEqual(2, len(self.store.list(vehicle=None)))

    def test_record_failure_raises_and_leaves_no_row(self):
        missing = Path(self._tmp.name) / "no-such.bin"
        with self.assertRaises(OSError):
            self.store.record("veh", "STM32F1", str(missing), "")
        self.assertEqual([], self.store.list(vehicle="veh"))


if __name__ == "__main__":
    unittest.main()
