"""Read-only live telemetry over HTTP + Server-Sent Events (stdlib only).

The race-day use case: the car is on track and a teammate watches live
curves on a phone browser at http://<pc-ip>:<port>/. The server is
strictly read-only - no control surface is exposed (the e-stop stays on
the desktop app), and it binds only when the user starts it from the UI.
"""

from __future__ import annotations

import json
import socket
import threading
from collections import deque
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

_HISTORY_LEN = 600          # ~1 分钟 @10Hz 的服务端历史
_CLIENT_QUEUE_MAX = 200

PAGE_HTML = """<!DOCTYPE html>
<html lang="zh"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>DiCAR LAB 赛道遥测</title>
<style>
body{margin:0;font-family:system-ui,sans-serif;background:#101418;color:#dfe7ef}
header{padding:10px 14px;font-weight:700;border-bottom:1px solid #232b33}
#wrap{padding:10px}canvas{width:100%;background:#0b0e11;border:1px solid #232b33;border-radius:6px}
#chips{display:flex;flex-wrap:wrap;gap:6px;padding:8px 0}
.chip{padding:4px 10px;border:1px solid #2c3540;border-radius:14px;cursor:pointer;font-size:13px}
.chip.on{background:#1d4ed8;border-color:#1d4ed8}
#state{padding:6px 14px;color:#8b98a5;font-size:13px}
</style></head><body>
<header>DiCAR LAB · 赛道遥测（只读）</header>
<div id="state">连接中…</div>
<div id="wrap"><canvas id="cv" height="360"></canvas><div id="chips"></div></div>
<script>
const buf=[];const MAX=600;const chosen=new Set();
const cv=document.getElementById("cv"),ctx=cv.getContext("2d");
const es=new EventSource("/events");
es.onopen=()=>document.getElementById("state").textContent="已连接";
es.onerror=()=>document.getElementById("state").textContent="连接断开，正在重试…";
es.onmessage=(ev)=>{const m=JSON.parse(ev.data);if(m.snapshot){buf.push(m);if(buf.length>MAX)buf.shift();
  if(document.getElementById("chips").childElementCount===0)buildChips(m.data);draw();}};
function buildChips(data){const box=document.getElementById("chips");
  Object.keys(data).slice(0,14).forEach(k=>{const c=document.createElement("span");
  c.className="chip";c.textContent=k;c.onclick=()=>{if(chosen.has(k))chosen.delete(k);else if(chosen.size<4)chosen.add(k);
  c.classList.toggle("on",chosen.has(k));draw();};box.appendChild(c);});}
function draw(){const w=cv.clientWidth;cv.width=w;ctx.clearRect(0,0,cv.width,cv.height);
  const colors=["#4d9fff","#5ad68e","#ffb84d","#ff6b6b"];let ci=0;
  for(const k of chosen){const pts=[];buf.forEach((m,i)=>{const v=m.data[k];
    if(typeof v==="number")pts.push([i,v]);});
    if(pts.length<2)continue;let lo=Infinity,hi=-Infinity;pts.forEach(p=>{lo=Math.min(lo,p[1]);hi=Math.max(hi,p[1]);});
    if(hi-lo<1e-9){lo-=1;hi+=1;}ctx.beginPath();ctx.strokeStyle=colors[ci++%4];ctx.lineWidth=1.6;
    pts.forEach((p,i)=>{const x=p[0]/(MAX-1)*cv.width,y=cv.height-(p[1]-lo)/(hi-lo)*(cv.height-16)-8;
    i?ctx.lineTo(x,y):ctx.moveTo(x,y);});ctx.stroke();}}
window.addEventListener("resize",draw);
</script></body></html>
"""


def _local_ips() -> list:
    ips = set()
    try:
        hostname = socket.gethostname()
        for info in socket.getaddrinfo(hostname, None, socket.AF_INET):
            ip = info[4][0]
            if not ip.startswith("127."):
                ips.add(ip)
    except OSError:
        pass
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
            sock.connect(("8.8.8.8", 80))
            ip = sock.getsockname()[0]
            if not ip.startswith("127."):
                ips.add(ip)
    except OSError:
        pass
    return sorted(ips)


class _Handler(BaseHTTPRequestHandler):
    server_version = "DiCAR-LAB/1.0"

    def log_message(self, *_args):  # 静默默认访问日志
        pass

    def _send(self, code, body, content_type):
        self.send_response(code)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        state = self.server.state
        if self.path == "/":
            self._send(200, PAGE_HTML.encode("utf-8"), "text/html; charset=utf-8")
        elif self.path == "/health":
            self._send(200, b"ok", "text/plain")
        elif self.path == "/events":
            self._serve_events(state)
        else:
            self._send(404, b"not found", "text/plain")

    def _serve_events(self, state):
        queue = state.register_client()
        try:
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.end_headers()
            for snapshot in state.history_snapshot():
                self.wfile.write(b"data: " + json.dumps(
                    snapshot, ensure_ascii=False).encode("utf-8") + b"\n\n")
            self.wfile.flush()
            while state.running:
                try:
                    item = queue.get(timeout=5.0)
                except Exception:
                    item = None  # 心跳
                if item is not None:
                    payload = json.dumps(item, ensure_ascii=False).encode("utf-8")
                    self.wfile.write(b"data: " + payload + b"\n\n")
                else:
                    self.wfile.write(b": keepalive\n\n")
                self.wfile.flush()
        except (ConnectionError, BrokenPipeError, OSError):
            pass
        finally:
            state.remove_client(queue)


class TelemetryServerState:
    """Holds the latest snapshot history and per-client queues."""

    def __init__(self):
        self.running = False
        self._history = deque(maxlen=_HISTORY_LEN)
        self._clients = []
        self._lock = threading.Lock()

    def publish(self, data: dict):
        snapshot = {"t": round(data.get("t", 0.0), 3) if isinstance(data.get("t"), (int, float)) else None,
                    "data": {k: v for k, v in data.items() if isinstance(v, (int, float))}}
        with self._lock:
            self._history.append(snapshot)
            clients = list(self._clients)
        for queue in clients:
            try:
                queue.put_nowait(snapshot)
            except Exception:
                pass  # 队列满则丢弃，实时性优先

    def register_client(self):
        queue = deque(maxlen=_CLIENT_QUEUE_MAX)
        with self._lock:
            self._clients.append(queue)
        return queue

    def remove_client(self, queue):
        with self._lock:
            if queue in self._clients:
                self._clients.remove(queue)

    @property
    def client_count(self) -> int:
        with self._lock:
            return len(self._clients)

    def history_snapshot(self):
        with self._lock:
            return list(self._history)


class TelemetryServer:
    def __init__(self, port: int = 8899):
        self.port = int(port)
        self.state = TelemetryServerState()
        self._httpd = None

    def start(self):
        if self._httpd is not None:
            return
        self.state.running = True
        self._httpd = ThreadingHTTPServer(("0.0.0.0", self.port), _Handler)
        self._httpd.state = self.state
        self._thread = threading.Thread(target=self._httpd.serve_forever, daemon=True)
        self._thread.start()

    def stop(self):
        self.state.running = False
        if self._httpd is not None:
            self._httpd.shutdown()
            self._httpd.server_close()
            self._httpd = None

    @property
    def running(self) -> bool:
        return self._httpd is not None

    @staticmethod
    def local_urls(port: int) -> list:
        return [f"http://{ip}:{port}/" for ip in _local_ips()]
