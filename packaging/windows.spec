# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for Windows — bundles mpv DLLs."""

import os
import glob

# Find mpv DLLs — installed by `choco install mpv`
_mpv_dlls = []
for pattern in [
    "C:\\ProgramData\\chocolatey\\lib\\mpv\\**\\mpv*.dll",
    "C:\\ProgramData\\chocolatey\\bin\\mpv*.dll",
    "C:\\tools\\mpv\\mpv*.dll",
    os.path.expandvars(r"%LOCALAPPDATA%\Programs\mpv\mpv*.dll"),
]:
    _mpv_dlls.extend(glob.glob(pattern, recursive=True))

# Also search PATH entries
for dir_entry in os.environ.get("PATH", "").split(os.pathsep):
    for name in ["mpv-1.dll", "mpv-2.dll", "libmpv-2.dll"]:
        candidate = os.path.join(dir_entry, name)
        if os.path.isfile(candidate):
            _mpv_dlls.append(candidate)

_binaries = []
for dll in sorted(set(_mpv_dlls)):
    real = os.path.realpath(dll)
    if real not in [b[0] for b in _binaries]:
        _binaries.append((real, os.path.basename(dll)))

a = Analysis(
    ['..\\src\\jorja_clipper\\app.py'],
    pathex=[],
    binaries=_binaries,
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
    a.binaries,
    a.datas,
    [],
    name='jorja-clipper',
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
