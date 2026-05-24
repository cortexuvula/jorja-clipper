# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for Linux — bundles libmpv."""

import glob
import os

# Find libmpv.so — installed by `apt install libmpv-dev`
_mpv_libs = []
for pattern in [
    "/usr/lib/x86_64-linux-gnu/libmpv.so*",
    "/usr/lib/libmpv.so*",
]:
    _mpv_libs.extend(glob.glob(pattern))

# Deduplicate: keep only actual files (not symlinks pointing outside)
_binaries = []
for lib in sorted(set(_mpv_libs)):
    real = os.path.realpath(lib)
    if real not in [b[0] for b in _binaries]:
        _binaries.append((real, os.path.basename(lib)))

SCRIPT = os.path.join(SPECPATH, "..", "src", "jorja_clipper", "app.py")
HOOK = os.path.join(SPECPATH, "runtime_hook_mpv.py")

a = Analysis(
    [SCRIPT],
    pathex=[],
    binaries=_binaries,
    datas=[],
    hiddenimports=["mpv"],
    hookspath=[],
    runtime_hooks=[HOOK],
    hooksconfig={},
    excludes=[],
    noarchive=False,
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name="jorja-clipper",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,
    disable_windowed_traceback=False,
    icon=None,
)
