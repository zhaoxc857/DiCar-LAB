# -*- mode: python ; coding: utf-8 -*-

from PyInstaller.utils.hooks import collect_submodules


analysis = Analysis(
    ["CAR_LAB/main.py"],
    pathex=["CAR_LAB"],
    binaries=[],
    datas=[
        ("CAR_LAB/vehicles", "vehicles"),
        ("CAR_LAB/docs", "docs"),
        ("CAR_LAB/examples", "examples"),
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
    [],
    exclude_binaries=True,
    name="DiCAR LAB",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)

bundle = COLLECT(
    exe,
    analysis.binaries,
    analysis.datas,
    strip=False,
    upx=True,
    upx_exclude=[],
    name="DiCAR LAB",
)
