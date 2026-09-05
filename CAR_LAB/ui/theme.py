"""Design tokens, QSS generation and plot theming.

Public API (unchanged for callers/tests):
    DARK_STYLE / LIGHT_STYLE / THEME_STYLES {"黑色","白色"}
    PLOT_THEMES / apply_plot_theme(root, theme_name) / make_ikun_icon(theme)

Everything else is generated: both styles come from one QSS template fed
by per-theme token dicts, so dark/light cannot drift and the wallpaper
(frosted-glass) mode can derive translucent rgba surfaces from the same
palette. Token colors are hex strings; _rgba() derives translucent
variants for wallpaper mode.
"""
from PySide6.QtGui import QColor, QFont, QIcon, QPainter, QPixmap
import pyqtgraph as pg


def _rgba(hex_color: str, alpha: int) -> str:
    c = QColor(hex_color)
    return f"rgba({c.red()},{c.green()},{c.blue()},{max(0, min(255, int(alpha)))})"


DARK_TOKENS = {
    "bg": "#0b1017", "window": "#080d13",
    "surface": "#111923", "surface2": "#0c131c", "surface3": "#182433",
    "border": "#223044", "border_strong": "#304157",
    "text": "#e7edf5", "muted": "#8494a7",
    "hover_surface": "#152131", "selected_surface": "#1a3954", "selected_text": "#ffffff",
    "accent": "#176ca4", "accent_hover": "#1e7bb8", "accent_pressed": "#0f5b8d",
    "accent_text": "#e7edf5", "focus": "#65a9df",
    "accent_disabled": "#18384d", "accent_disabled_text": "#b8cfdb",
    "success": "#78d79a", "warn": "#e8c268", "danger": "#e5737a",
    "good_bg": "#10281b", "good_border": "#245d3b",
    "bad_bg": "#2c1517", "bad_border": "#683034",
    "danger_btn": "#67252b", "danger_btn_hover": "#7d2c34",
    "danger_btn_pressed": "#521b21", "danger_btn_border": "#a23b45",
    "danger_disabled": "#331b1e", "danger_disabled_text": "#cfa5a8",
    "scrim": "#05080d", "scrim_alpha": 118,
    "plot_bg": "#0a0f16", "plot_fg": "#b9c4d0", "plot_axis": "#5f6c79",
    "curves": ["#5aa2ff", "#ffb454", "#ff7a86", "#5ad69e",
               "#b48cf2", "#45c4c9", "#ffd166", "#9aa7ff"],
}

LIGHT_TOKENS = {
    "bg": "#f5f6f8", "window": "#eef1f4",
    "surface": "#ffffff", "surface2": "#ffffff", "surface3": "#f1f5f9",
    "border": "#d7dce2", "border_strong": "#cbd2da",
    "text": "#20242a", "muted": "#68727f",
    "hover_surface": "#f0f4f8", "selected_surface": "#dcedf8", "selected_text": "#0b5f91",
    "accent": "#1674ae", "accent_hover": "#0f659a", "accent_pressed": "#0b5684",
    "accent_text": "#ffffff", "focus": "#07517e",
    "accent_disabled": "#a9c9dc", "accent_disabled_text": "#243746",
    "success": "#18733b", "warn": "#9a6700", "danger": "#c33c46",
    "good_bg": "#edf9f1", "good_border": "#9bd2ad",
    "bad_bg": "#fff1f2", "bad_border": "#e2a8ad",
    "danger_btn": "#c33c46", "danger_btn_hover": "#ab2e37",
    "danger_btn_pressed": "#92242d", "danger_btn_border": "#b4313b",
    "danger_disabled": "#e1a5aa", "danger_disabled_text": "#5d2227",
    "scrim": "#ffffff", "scrim_alpha": 150,
    "plot_bg": "#ffffff", "plot_fg": "#30363d", "plot_axis": "#7b8794",
    "curves": ["#1d6fd1", "#d97706", "#d64550", "#14935f",
               "#7c4dcc", "#0e9aa7", "#b8860b", "#5c6bc0"],
}

TOKENS = {"黑色": DARK_TOKENS, "白色": LIGHT_TOKENS}

# 当前主题名（main_window 切换主题时经 apply_plot_theme 更新；供
# plot_cursor / state_color 等无法接收参数的调用点读取）。
current_theme = "白色"

# 统一几何/字号刻度
RADIUS_CONTAINER = 10   # 面板/卡片/侧栏
RADIUS_CONTROL = 7      # 按钮/输入框/列表项
RADIUS_PILL = 11        # 状态胶囊
FONT_STAT = 28          # 数据大数字
MARGIN = 10             # 页面根边距 / 布局间距


def tokens(theme_name: str) -> dict:
    return TOKENS.get(theme_name, TOKENS["白色"])


def set_current_theme(theme_name: str) -> None:
    global current_theme
    current_theme = theme_name if theme_name in TOKENS else "白色"


def state_color(name: str) -> str:
    """ok/success、warn、bad/danger → 当前主题下的状态色 hex。"""
    t = tokens(current_theme)
    return {"ok": t["success"], "success": t["success"],
            "warn": t["warn"], "bad": t["danger"],
            "danger": t["danger"]}.get(str(name).lower(), t["text"])


def plot_color(index: int) -> str:
    curves = tokens(current_theme)["curves"]
    return curves[int(index) % len(curves)]


def plot_pen(index: int, width: int = 2, style=None):
    return pg.mkPen(plot_color(index), width=width) if style is None \
        else pg.mkPen(plot_color(index), width=width, style=style)


def build_qss(theme_name: str, wallpaper: bool = False,
              panel_opacity: int = 78) -> str:
    """Generate the full application stylesheet from tokens.

    wallpaper=False → opaque surfaces (classic look).
    wallpaper=True  → global background turns transparent so the central
    wallpaper canvas shows through; panels/inputs become translucent
    rgba surfaces at `panel_opacity` percent.
    """
    t = tokens(theme_name)
    set_current_theme(theme_name)
    r_c, r_n, r_p = RADIUS_CONTAINER, RADIUS_CONTROL, RADIUS_PILL
    op = max(0, min(100, int(panel_opacity)))
    a_panel = round(op / 100 * 255)
    a_input = min(255, a_panel + 45)      # 输入类略微更实，保证可读
    a_menu = min(255, a_panel + 60)

    panel_bg = _rgba(t["surface"], a_panel) if wallpaper else t["surface"]
    input_bg = _rgba(t["surface2"], a_input) if wallpaper else t["surface2"]
    group_bg = panel_bg
    if wallpaper:
        base = (f"QWidget {{ background: transparent; color:{t['text']}; "
                f'font-family:"Microsoft YaHei UI","Segoe UI"; font-size:13px; }}\n'
                f"QMainWindow {{ background: transparent; }}")
    else:
        base = (f"QWidget {{ background:{t['bg']}; color:{t['text']}; "
                f'font-family:"Microsoft YaHei UI","Segoe UI"; font-size:13px; }}\n'
                f"QMainWindow {{ background:{t['window']}; }}")

    return f"""
{base}
QDialog, QMessageBox {{ background:{t['bg']}; }}
QFrame#header, QFrame#panel, QFrame#toolbar, QFrame#card {{
    background:{panel_bg}; border:1px solid {t['border']}; border-radius:{r_c}px;
}}
QFrame#sidebar {{ background:{panel_bg}; border-right:1px solid {t['border']}; }}
QLabel#brandDiCAR {{ font-size:24px; font-weight:900; font-style:italic; letter-spacing:1px; color:{t['text']}; }}
QLabel#subtitle, QLabel#muted {{ color:{t['muted']}; }}
QLabel#panelTitle {{ color:{t['text']}; font-size:14px; font-weight:700; }}
QLabel#pageTitle {{ color:{t['text']}; font-size:18px; font-weight:750; }}
QLabel#statValue {{ font-size:{FONT_STAT}px; font-weight:700; color:{t['text']}; }}
QLabel#statusGood {{ color:{t['success']}; background:{t['good_bg']}; border:1px solid {t['good_border']}; border-radius:{r_p}px; padding:3px 9px; }}
QLabel#statusBad {{ color:{t['danger']}; background:{t['bad_bg']}; border:1px solid {t['bad_border']}; border-radius:{r_p}px; padding:3px 9px; }}
QPushButton {{ background:{t['surface3']}; border:1px solid {t['border_strong']}; border-radius:{r_n}px; padding:7px 11px; color:{t['text']}; }}
QPushButton:hover {{ background:{t['hover_surface']}; border-color:{t['accent']}; }}
QPushButton:pressed {{ background:{t['surface2']}; border-color:{t['focus']}; padding-top:8px; padding-bottom:6px; }}
QPushButton:focus {{ border-width:2px; border-color:{t['focus']}; }}
QPushButton:checked {{ background:{t['selected_surface']}; border-color:{t['accent']}; }}
QPushButton:disabled {{ color:{t['muted']}; background:{t['surface']}; border-color:{t['border']}; }}
QPushButton#primary {{ background:{t['accent']}; border-color:{t['accent']}; color:{t['accent_text']}; font-weight:700; }}
QPushButton#primary:hover {{ background:{t['accent_hover']}; }}
QPushButton#primary:pressed {{ background:{t['accent_pressed']}; border-color:{t['accent_pressed']}; padding-top:8px; padding-bottom:6px; }}
QPushButton#primary:focus {{ border-width:2px; border-color:{t['focus']}; }}
QPushButton#primary:disabled {{ color:{t['accent_disabled_text']}; background:{t['accent_disabled']}; border-color:{t['accent_disabled']}; }}
QPushButton#danger {{ background:{t['danger_btn']}; border-color:{t['danger_btn_border']}; color:{t['accent_text']}; font-weight:700; }}
QPushButton#danger:hover {{ background:{t['danger_btn_hover']}; }}
QPushButton#danger:pressed {{ background:{t['danger_btn_pressed']}; border-color:{t['danger_btn_pressed']}; padding-top:8px; padding-bottom:6px; }}
QPushButton#danger:focus {{ border-width:2px; border-color:{t['danger']}; }}
QPushButton#danger:disabled {{ color:{t['danger_disabled_text']}; background:{t['danger_disabled']}; border-color:{t['danger_disabled']}; }}
QToolButton {{
    background:{t['surface3']}; border:1px solid {t['border_strong']};
    border-radius:{r_n}px; padding:5px 9px; color:{t['text']};
}}
QToolButton:hover {{ background:{t['hover_surface']}; border-color:{t['accent']}; }}
QToolButton:pressed {{ background:{t['surface2']}; border-color:{t['focus']}; }}
QToolButton:disabled {{ color:{t['muted']}; background:{t['surface']}; border-color:{t['border']}; }}
QLineEdit, QComboBox, QSpinBox, QDoubleSpinBox, QTableWidget, QTextEdit, QPlainTextEdit {{
    background:{input_bg}; color:{t['text']}; border:1px solid {t['border_strong']};
    border-radius:{r_n}px; padding:5px; selection-background-color:{t['selected_surface']}; selection-color:{t['selected_text']};
}}
QComboBox::drop-down {{ border:0; width:24px; }}
QComboBox::down-arrow {{ subcontrol-origin:padding; subcontrol-position:center right; right:7px; width:0; height:0; border-left:4px solid transparent; border-right:4px solid transparent; border-top:5px solid {t['muted']}; }}
QComboBox QAbstractItemView {{
    background:{_rgba(t['surface2'], a_menu) if wallpaper else t['surface2']}; color:{t['text']};
    border:1px solid {t['border_strong']}; border-radius:{r_n}px; outline:0;
    selection-background-color:{t['accent']}; selection-color:{t['accent_text']};
}}
QSpinBox::up-button, QDoubleSpinBox::up-button,
QSpinBox::down-button, QDoubleSpinBox::down-button {{ width:18px; border:0; background:transparent; }}
QSpinBox::up-button:hover, QDoubleSpinBox::up-button:hover,
QSpinBox::down-button:hover, QDoubleSpinBox::down-button:hover {{ background:{t['hover_surface']}; border-radius:4px; }}
QSpinBox::up-arrow, QDoubleSpinBox::up-arrow {{ width:0; height:0; border-left:3px solid transparent; border-right:3px solid transparent; border-bottom:4px solid {t['muted']}; }}
QSpinBox::down-arrow, QDoubleSpinBox::down-arrow {{ width:0; height:0; border-left:3px solid transparent; border-right:3px solid transparent; border-top:4px solid {t['muted']}; }}
QCheckBox, QRadioButton {{ spacing:6px; }}
QCheckBox::indicator, QRadioButton::indicator, QAbstractItemView::indicator {{
    width:15px; height:15px; border:1px solid {t['border_strong']}; background:{input_bg};
}}
QCheckBox::indicator {{ border-radius:4px; }}
QRadioButton::indicator {{ border-radius:8px; }}
QCheckBox::indicator:hover, QRadioButton::indicator:hover, QAbstractItemView::indicator:hover {{ border-color:{t['accent']}; }}
QCheckBox::indicator:checked, QAbstractItemView::indicator:checked {{ background:{t['accent']}; border-color:{t['accent']}; }}
QRadioButton::indicator:checked {{ background:{input_bg}; border:5px solid {t['accent']}; }}
QCheckBox::indicator:disabled, QRadioButton::indicator:disabled, QAbstractItemView::indicator:disabled {{ background:{t['surface']}; border-color:{t['border']}; }}
QMenu {{
    background:{_rgba(t['surface2'], a_menu) if wallpaper else t['surface2']}; color:{t['text']};
    border:1px solid {t['border_strong']}; border-radius:8px; padding:4px;
}}
QMenu::item {{ padding:6px 22px 6px 12px; border-radius:6px; }}
QMenu::item:selected {{ background:{t['accent']}; color:{t['accent_text']}; }}
QMenu::item:disabled {{ color:{t['muted']}; }}
QMenu::separator {{ height:1px; background:{t['border']}; margin:4px 6px; }}
QListWidget {{
    background:{input_bg}; color:{t['text']}; border:1px solid {t['border_strong']};
    border-radius:{r_c}px; outline:0; padding:4px;
}}
QListWidget::item {{ border-radius:6px; padding:5px 8px; color:{t['text']}; }}
QListWidget::item:hover {{ background:{t['hover_surface']}; }}
QListWidget::item:selected {{ background:{t['selected_surface']}; color:{t['selected_text']}; }}
QTableWidget {{ gridline-color:{t['border']}; alternate-background-color:{_rgba(t['surface3'], a_panel) if wallpaper else t['surface3']}; }}
QHeaderView::section {{ background:{t['surface3']}; color:{t['muted']}; padding:6px; border:0; border-bottom:1px solid {t['border_strong']}; }}
QGroupBox {{ border:1px solid {t['border']}; border-radius:{r_c}px; margin-top:12px; padding-top:9px; font-weight:650; background:{group_bg}; }}
QGroupBox::title {{ subcontrol-origin:margin; left:10px; padding:0 5px; color:{t['muted']}; }}
QTreeWidget {{ background:transparent; border:0; outline:0; padding:5px; alternate-background-color:{_rgba(t['surface3'], a_panel) if wallpaper else t['surface3']}; }}
QTreeWidget::item {{ height:35px; border-radius:{r_n}px; padding-left:4px; color:{t['text']}; }}
QTreeWidget::item:hover {{ background:{t['hover_surface']}; }}
QTreeWidget::item:selected {{ background:{t['selected_surface']}; color:{t['selected_text']}; }}
QSplitter::handle {{ background:{panel_bg}; width:4px; }}
QProgressBar {{ background:{t['surface2']}; border:1px solid {t['border']}; border-radius:6px; text-align:center; color:{t['text']}; }}
QProgressBar::chunk {{ background:{t['accent']}; border-radius:5px; }}
QToolTip {{ background:{t['surface2']}; color:{t['text']}; border:1px solid {t['border_strong']}; padding:4px; }}
QScrollBar:vertical {{ background:transparent; width:10px; margin:0; }}
QScrollBar::handle:vertical {{ background:{t['border_strong']}; min-height:26px; border-radius:5px; }}
QScrollBar:horizontal {{ background:transparent; height:10px; }}
QScrollBar::handle:horizontal {{ background:{t['border_strong']}; min-width:26px; border-radius:5px; }}
QScrollBar::add-line, QScrollBar::sub-line {{ width:0; height:0; background:none; border:0; }}
QAbstractScrollArea::corner {{ background:transparent; }}
""".strip()


DARK_STYLE = build_qss("黑色")
LIGHT_STYLE = build_qss("白色")
THEME_STYLES = {"黑色": DARK_STYLE, "白色": LIGHT_STYLE}

PLOT_THEMES = {
    name: {"bg": t["plot_bg"], "fg": t["plot_fg"], "axis": t["plot_axis"]}
    for name, t in TOKENS.items()
}


def apply_plot_theme(root, theme_name: str):
    cfg = PLOT_THEMES.get(theme_name, PLOT_THEMES["黑色"])
    set_current_theme(theme_name)
    fg = QColor(cfg["fg"])
    axis_color = QColor(cfg["axis"])
    for plot in root.findChildren(pg.PlotWidget):
        plot.setBackground(cfg["bg"])
        item = plot.getPlotItem()
        for axis_name in ("left", "right", "bottom", "top"):
            try:
                axis = item.getAxis(axis_name)
                axis.setPen(pg.mkPen(axis_color))
                axis.setTextPen(pg.mkPen(fg))
            except Exception:
                pass
        try:
            item.titleLabel.item.setDefaultTextColor(fg)
        except Exception:
            pass
        try:
            if item.legend:
                for _sample, label in item.legend.items:
                    label.item.setDefaultTextColor(fg)
        except Exception:
            pass


def make_ikun_icon(theme_name: str) -> QIcon:
    light = theme_name == "白色"
    pix = QPixmap(192, 192)
    pix.fill(QColor(0, 0, 0, 0))
    painter = QPainter(pix)
    painter.setRenderHint(QPainter.RenderHint.Antialiasing, True)
    bg = QColor("#ffffff" if light else "#101820")
    fg = QColor("#111827" if light else "#f8fafc")
    painter.setBrush(bg)
    painter.setPen(QColor("#cdd3da" if light else "#304255"))
    painter.drawRoundedRect(7, 7, 178, 178, 34, 34)
    font = QFont("Arial", 42, QFont.Weight.Black)
    font.setItalic(True)
    painter.setFont(font)
    painter.setPen(fg)
    painter.drawText(pix.rect(), 0x84, "DiCAR")  # AlignCenter
    painter.end()
    return QIcon(pix)
