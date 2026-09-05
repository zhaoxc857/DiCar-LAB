import json
import os
import sys
import unittest
import urllib.request
from pathlib import Path


os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core.telemetry_server import TelemetryServer


class TelemetryServerTests(unittest.TestCase):
    def setUp(self):
        self.server = TelemetryServer(port=18899)
        self.addCleanup(self.server.stop)

    def _get(self, path):
        with urllib.request.urlopen(f"http://127.0.0.1:18899{path}", timeout=3) as resp:
            return resp.status, resp.read().decode("utf-8")

    def test_serves_html_and_health(self):
        self.server.start()
        status, body = self._get("/")
        self.assertEqual(200, status)
        self.assertIn("DiCAR LAB", body)
        self.assertIn("EventSource", body)
        status, body = self._get("/health")
        self.assertEqual(200, status)
        self.assertEqual("ok", body)

    def test_publish_updates_history_and_snapshot_is_valid_json(self):
        self.server.start()
        self.server.state.publish({"battery": 12.0, "speed": 1.5, "name": "忽略非数值"})
        history = self.server.state.history_snapshot()
        self.assertEqual(1, len(history))
        snapshot = history[0]
        self.assertEqual({"battery": 12.0, "speed": 1.5}, snapshot["data"])
        json.dumps(snapshot)  # 必须可序列化给 SSE 客户端

    def test_stop_closes_server(self):
        self.assertFalse(self.server.running)
        self.server.start()
        self.assertTrue(self.server.running)
        self.server.stop()
        self.assertFalse(self.server.running)

    def test_local_urls_use_configured_port(self):
        urls = TelemetryServer.local_urls(12345)
        self.assertTrue(all(url.endswith(":12345/") for url in urls))


if __name__ == "__main__":
    unittest.main()
