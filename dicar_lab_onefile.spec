# -*- mode: python ; coding: utf-8 -*-
# Single-file build: one standalone DiCAR LAB.exe that self-extracts at
# startup. Read-only resources live in sys._MEIPASS; user data is written
# to %LOCALAPPDATA%/DiCAR LAB (see core/paths.py).

from PyInstaller.utils.hooks import collect_submodules


analysis = Analysis(
    ["CAR_LAB/main.py"],
    pathex=["CAR_LAB"],
    binaries=[],
    datas=[
        ("CAR_LAB/vehicles", "vehicles"),
        ("CAR_LAB/docs", "docs"),
        ("CAR_LAB/examples", "examples"),
        ("tools/stm32flash.exe", "tools"),
    ],
    hiddenimports=collect_submodules("bleak.backends"),
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(analysis.pure)

exe = EXE(
    pyz,
    analysis.scripts,
    analysis.binaries,
    analysis.datas,
    [],
    name="DiCAR LAB",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    runtime_tmpdir=None,
    console=False,
    icon="assets/app.ico",
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
