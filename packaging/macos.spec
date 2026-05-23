# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for macOS."""

a = Analysis(
    ['../src/jorja_clipper/app.py'],
    pathex=[],
    binaries=[],
    datas=[],
    hiddenimports=['mpv'],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name='jorja-clipper',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    console=False,
    disable_windowed_traceback=False,
)

coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=True,
    upx_exclude=[],
    name='jorja-clipper',
)

app = BUNDLE(
    coll,
    name='Jorja Clipper.app',
    icon=None,
    bundle_identifier='com.cortexuvula.jorja-clipper',
)
