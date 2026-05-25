# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for Windows — bundles mpv DLLs."""

import glob
import os

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

# Find ffmpeg — installed by `choco install ffmpeg` or manually
_ffmpeg_path = None
for dir_entry in os.environ.get("PATH", "").split(os.pathsep):
    candidate = os.path.join(dir_entry, "ffmpeg.exe")
    if os.path.isfile(candidate):
        _ffmpeg_path = candidate
        break

# Also check common installation paths
if not _ffmpeg_path:
    for path in [
        "C:\\ProgramData\\chocolatey\\bin\\ffmpeg.exe",
        "C:\\tools\\ffmpeg\\ffmpeg.exe",
        "C:\\ffmpeg\\bin\\ffmpeg.exe",
    ]:
        if os.path.exists(path):
            _ffmpeg_path = path
            break

# Find ffmpeg dependencies (av*.dll, sw*.dll)
_ffmpeg_dlls = []
if _ffmpeg_path:
    ffmpeg_dir = os.path.dirname(_ffmpeg_path)
    for pattern in [
        os.path.join(ffmpeg_dir, "av*.dll"),
        os.path.join(ffmpeg_dir, "sw*.dll"),
    ]:
        _ffmpeg_dlls.extend(glob.glob(pattern))

_binaries = []
for dll in sorted(set(_mpv_dlls)):
    real = os.path.realpath(dll)
    if real not in [b[0] for b in _binaries]:
        _binaries.append((real, os.path.basename(dll)))

# Add ffmpeg binary
if _ffmpeg_path:
    _binaries.append((_ffmpeg_path, "ffmpeg.exe"))

# Add ffmpeg dependencies
for dll in sorted(set(_ffmpeg_dlls)):
    real = os.path.realpath(dll)
    if real not in [b[0] for b in _binaries]:
        _binaries.append((real, os.path.basename(dll)))

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
