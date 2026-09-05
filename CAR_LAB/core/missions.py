"""Simulator tuning missions: small gamified exercises that run against the
built-in simulator car, evaluated with the same step-response metrics used
by the AI tuner. Pure evaluation logic lives here; the page drives the flow.
"""

from __future__ import annotations

from core.metrics import response_metrics


def _le(metric, bound):
    if metric is None:
        return False
    return float(metric) <= bound


def _abs_le(value, bound):
    return value is not None and abs(float(value)) <= bound


MISSIONS = [
    {
        "key": "speed_overshoot",
        "title": "关卡 1 · 驯服超调",
        "brief": "仿真车速度环 Kp 偏大，目标 500 RPM 时超调明显。请调低 Kp（必要时微调 Kd），"
                 "让超调 ≤ 15% 且稳态误差 ≤ 3 RPM。",
        "loop": "speed",
        "param_setup": {"speed_kp": 1.6, "speed_ki": 0.12, "speed_kd": 0.0},
        "step": {"key": "target_rpm", "value": 500.0},
        "collect_s": 4.0,
        "sample_key": "actual_rpm",
        "target_key": "target_rpm",
        "rules": [("overshoot_pct", 15.0), ("steady_error", 3.0)],
        "stars": [("overshoot_pct", 8.0), ("steady_error", 1.5)],
    },
    {
        "key": "speed_response",
        "title": "关卡 2 · 提速响应",
        "brief": "这次 Kp 偏小，响应磨蹭。在不产生明显超调（≤ 25%）的前提下把上升时间压到 0.8s 以内。",
        "loop": "speed",
        "param_setup": {"speed_kp": 0.35, "speed_ki": 0.05, "speed_kd": 0.0},
        "step": {"key": "target_rpm", "value": 500.0},
        "collect_s": 4.0,
        "sample_key": "actual_rpm",
        "target_key": "target_rpm",
        "rules": [("rise_time_s", 0.8), ("overshoot_pct", 25.0)],
        "stars": [("rise_time_s", 0.45), ("overshoot_pct", 12.0)],
    },
    {
        "key": "heading_settle",
        "title": "关卡 3 · 航向收敛",
        "brief": "让车头阶跃 90° 后快速稳定：整定时间 ≤ 2.5s，稳态误差 ≤ 5°。外环内环都可以动。",
        "loop": "heading",
        "param_setup": {"heading_kp": 0.8, "heading_ki": 0.0, "heading_kd": 0.05},
        "step": {"key": "target_yaw", "value": 90.0},
        "collect_s": 5.0,
        "sample_key": "yaw",
        "target_key": "target_yaw",
        "rules": [("settling_time_s", 2.5), ("steady_error", 5.0)],
        "stars": [("settling_time_s", 1.5), ("steady_error", 2.0)],
    },
]


def evaluate_mission(mission: dict, samples: list) -> dict:
    """Run the step metrics and judge the rules.

    Returns {"metrics", "passed", "stars"} - stars are 0 when failed,
    1 when all base rules pass, 2 when the stricter star rules also pass.
    """
    metrics = response_metrics(
        samples, target_key=mission.get("target_key", "target"),
        actual_key=mission.get("sample_key", "actual"),
    )
    if not metrics:
        return {"metrics": {}, "passed": False, "stars": 0,
                "detail": "样本不足，未形成有效阶跃"}
    detail = []
    passed = True
    for metric_name, bound in mission.get("rules", []):
        value = metrics.get(metric_name)
        ok = _le(value, bound)
        passed = passed and ok
        detail.append(f"{metric_name}={value if value is None else round(float(value), 3)}（要求 ≤ {bound}）{'✓' if ok else '✗'}")
    stars = 0
    if passed:
        stars = 1
        if all(_le(metrics.get(n), b) for n, b in mission.get("stars", [])):
            stars = 2
    return {"metrics": metrics, "passed": passed, "stars": stars, "detail": detail}
