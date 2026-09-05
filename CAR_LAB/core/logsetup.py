"""Lightweight logging setup: level via DICAR_LOG_LEVEL env, file under
data_root()/logs/dicar_lab.log, mirrored to stderr."""

from __future__ import annotations

import logging
import os

from core.paths import data_root


def setup_logging() -> None:
    root = logging.getLogger()
    if root.handlers:
        return
    level_name = os.environ.get("DICAR_LOG_LEVEL", "INFO").upper()
    root.setLevel(getattr(logging, level_name, logging.INFO))
    formatter = logging.Formatter(
        "%(asctime)s %(levelname)s %(name)s: %(message)s"
    )
    try:
        log_dir = data_root() / "logs"
        log_dir.mkdir(parents=True, exist_ok=True)
        file_handler = logging.FileHandler(
            log_dir / "dicar_lab.log", encoding="utf-8"
        )
        file_handler.setFormatter(formatter)
        root.addHandler(file_handler)
    except OSError:
        pass  # 不可写环境（只读盘/权限）退化为仅控制台输出
    stream_handler = logging.StreamHandler()
    stream_handler.setFormatter(formatter)
    root.addHandler(stream_handler)
