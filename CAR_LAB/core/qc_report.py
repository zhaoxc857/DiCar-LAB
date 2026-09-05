"""HTML report builder for the vehicle delivery (QC) checklist.

Pure function - takes structured check results and returns a standalone
HTML document (no external assets), written to data_root()/reports/.
"""

from __future__ import annotations

import time


def build_qc_report_html(vehicle: str, items: list, fw_version: str = "",
                         operator: str = "") -> str:
    """items: list of {"name", "kind" ("auto"/"manual"), "state"
    ("pass"/"fail"/"pending"/"skip"), "detail"}."""
    state_text = {"pass": "通过", "fail": "未通过", "pending": "未完成", "skip": "跳过"}
    passed = sum(1 for i in items if i.get("state") == "pass")
    failed = sum(1 for i in items if i.get("state") == "fail")
    overall = "通过" if failed == 0 and passed == len(items) and items else "未通过"
    rows = []
    for item in items:
        state = str(item.get("state", "pending"))
        color = {"pass": "#1a7f37", "fail": "#b42318"}.get(state, "#8a6d00")
        rows.append(
            f"<tr><td>{item.get('name', '')}</td>"
            f"<td>{'自动' if item.get('kind') == 'auto' else '人工'}</td>"
            f"<td style='color:{color};font-weight:700'>{state_text.get(state, state)}</td>"
            f"<td>{item.get('detail', '')}</td></tr>"
        )
    fw = fw_version or "未上报（旧固件或未连接）"
    return f"""<!DOCTYPE html>
<html lang="zh"><head><meta charset="utf-8"><title>DiCAR LAB 整车下线检查报告</title>
<style>
body{{font-family:system-ui,'Microsoft YaHei',sans-serif;margin:24px;color:#1f2430}}
h1{{font-size:20px}} table{{border-collapse:collapse;width:100%;margin-top:12px}}
td,th{{border:1px solid #d7dce2;padding:8px 10px;text-align:left;font-size:14px}}
th{{background:#f2f5f8}} .overall{{font-size:26px;font-weight:800;
color:{'#1a7f37' if overall == '通过' else '#b42318'}}}
.meta{{color:#5b6570;font-size:13px;margin-top:6px}}
</style></head><body>
<h1>整车下线检查报告</h1>
<div class="overall">结论：{overall}</div>
<div class="meta">车型：{vehicle} ｜ 固件版本：{fw} ｜ 检查员：{operator or '—'} ｜
时间：{time.strftime('%Y-%m-%d %H:%M:%S')} ｜ 自动项通过 {passed}/{len(items)}，未通过 {failed}</div>
<table><tr><th>检查项</th><th>方式</th><th>结果</th><th>数据 / 说明</th></tr>
{''.join(rows)}
</table>
<div class="meta">本报告由 DiCAR LAB 本地生成；自动项仅反映遥测时刻状态，人工项以检查员确认为准。</div>
</body></html>"""
