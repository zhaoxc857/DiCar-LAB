"""Appearance preferences: custom wallpaper + frosted-glass tuning.

Settings live in QSettings under ``appearance/*``. The blur pipeline
renders once per (path, radius) and caches, so slider drags and window
resizes stay cheap. With no wallpaper set every getter falls back and the
app looks exactly like the plain theme.
"""

from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import QRectF, Qt
from PySide6.QtGui import QColor, QImage, QPainter, QPixmap
from PySide6.QtWidgets import (
    QGraphicsBlurEffect,
    QGraphicsPixmapItem,
    QGraphicsScene,
)

BLUR_MAX = 32
OPACITY_MIN = 55
OPACITY_MAX = 100

DEFAULTS = {"wallpaper_path": "", "blur_radius": 12, "panel_opacity": 78}

_blur_cache: dict = {}      # (path, radius) -> QPixmap
_scaled_cache: dict = {}    # (path, radius, w, h) -> QPixmap
_CACHE_MAX = 8


def load_appearance(settings) -> dict:
    try:
        blur = int(settings.value("appearance/blur_radius", DEFAULTS["blur_radius"]))
        opacity = int(settings.value("appearance/panel_opacity", DEFAULTS["panel_opacity"]))
    except (TypeError, ValueError):
        blur, opacity = DEFAULTS["blur_radius"], DEFAULTS["panel_opacity"]
    return {
        "wallpaper_path": str(settings.value("appearance/wallpaper_path", "") or ""),
        "blur_radius": max(0, min(BLUR_MAX, blur)),
        "panel_opacity": max(OPACITY_MIN, min(OPACITY_MAX, opacity)),
    }


def save_appearance(settings, appearance: dict) -> None:
    for key, value in appearance.items():
        settings.setValue(f"appearance/{key}", value)


def wallpaper_active(appearance: dict) -> bool:
    path = str(appearance.get("wallpaper_path", ""))
    return bool(path) and Path(path).is_file()


def _blur_source(path: str, radius: int):
    key = (str(path), int(radius))
    if key in _blur_cache:
        return _blur_cache[key]
    img = QImage(path)
    if img.isNull():
        return None
    source = QPixmap.fromImage(img)
    if radius > 0:
        pad = radius * 2  # 渲染区外扩，规避模糊边缘的透明晕圈，再裁回原尺寸
        scene = QGraphicsScene()
        item = QGraphicsPixmapItem(source)
        effect = QGraphicsBlurEffect()
        effect.setBlurRadius(radius)
        effect.setBlurHints(QGraphicsBlurEffect.BlurHint.QualityHint)
        item.setGraphicsEffect(effect)
        scene.addItem(item)
        canvas = QPixmap(source.width() + pad * 2, source.height() + pad * 2)
        canvas.fill(QColor("#000000"))
        painter = QPainter(canvas)
        scene.render(
            painter,
            target=QRectF(0, 0, canvas.width(), canvas.height()),
            source=QRectF(-pad, -pad, canvas.width(), canvas.height()),
        )
        painter.end()
        result = canvas.copy(pad, pad, source.width(), source.height())
    else:
        result = source
    if len(_blur_cache) >= _CACHE_MAX:
        _blur_cache.clear()
    _blur_cache[key] = result
    return result


def wallpaper_pixmap(path: str, radius: int, width: int, height: int):
    """Cover-fit blurred wallpaper at the requested size, or None."""
    path = str(path or "")
    if not path or not Path(path).is_file():
        return None
    width = max(1, int(width))
    height = max(1, int(height))
    key = (path, int(radius), width, height)
    if key in _scaled_cache:
        return _scaled_cache[key]
    blurred = _blur_source(path, int(radius))
    if blurred is None:
        return None
    scale = max(width / blurred.width(), height / blurred.height())
    scaled = blurred.scaled(
        round(blurred.width() * scale) + 1, round(blurred.height() * scale) + 1,
        Qt.AspectRatioMode.IgnoreAspectRatio,
        Qt.TransformationMode.SmoothTransformation,
    )
    x = (scaled.width() - width) // 2
    y = (scaled.height() - height) // 2
    result = scaled.copy(x, y, width, height)
    if len(_scaled_cache) >= _CACHE_MAX:
        _scaled_cache.clear()
    _scaled_cache[key] = result
    return result


def import_wallpaper(source: str, target_dir: Path) -> str:
    """Copy the picked image into data_root/theme/ so it survives moves."""
    import shutil
    import time as _time

    target_dir.mkdir(parents=True, exist_ok=True)
    ext = Path(source).suffix.lower() or ".png"
    target = target_dir / f"wallpaper_{_time.strftime('%Y%m%d_%H%M%S')}{ext}"
    shutil.copyfile(source, target)
    return str(target)
