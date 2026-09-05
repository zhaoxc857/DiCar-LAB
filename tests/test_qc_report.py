import os
import sys
import unittest
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core.qc_report import build_qc_report_html


class QcReportTests(unittest.TestCase):
    def test_report_contains_items_and_verdict(self):
        items = [
            {"name": "通信质量", "kind": "auto", "state": "pass", "detail": "实测 45.2 Hz"},
            {"name": "电池电压", "kind": "auto", "state": "fail", "detail": "最低 9.9 V"},
            {"name": "电机方向", "kind": "manual", "state": "pass", "detail": "已勾选"},
        ]
        html = build_qc_report_html("测试车", items, fw_version="dctp-1.1.0", operator="张三")
        for token in ("通信质量", "45.2 Hz", "dctp-1.1.0", "张三", "未通过", "9.9 V"):
            self.assertIn(token, html)
        self.assertEqual(1, html.count("<!DOCTYPE html>"))

    def test_all_pass_gives_green_verdict(self):
        items = [{"name": "a", "kind": "auto", "state": "pass", "detail": "ok"}]
        html = build_qc_report_html("v", items)
        self.assertIn("结论：通过", html)

    def test_pending_items_block_the_verdict(self):
        items = [
            {"name": "a", "kind": "auto", "state": "pass", "detail": "ok"},
            {"name": "b", "kind": "manual", "state": "pending", "detail": ""},
        ]
        html = build_qc_report_html("v", items)
        self.assertIn("结论：未通过", html)


if __name__ == "__main__":
    unittest.main()
