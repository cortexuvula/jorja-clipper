# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for macOS — bundles libmpv."""

import glob
import os

# Find libmpv.dylib — installed by `brew install mpv`
_mpv_libs = []
for pattern in [
    "/opt/homebrew/lib/libmpv*.dylib",   # Apple Silicon
    "/usr/local/lib/libmpv*.dylib",      # Intel
]:
    _mpv_libs.extend(glob.glob(pattern))

# Find ffmpeg — installed by `brew install ffmpeg`
_ffmpeg_path = None
for path in [
    "/opt/homebrew/bin/ffmpeg",   # Apple Silicon
    "/usr/local/bin/ffmpeg",      # Intel
]:
    if os.path.exists(path):
        _ffmpeg_path = os.path.realpath(path)  # Resolve symlinks
        print(f"Found ffmpeg at: {_ffmpeg_path}")
        break

if not _ffmpeg_path:
    print("WARNING: ffmpeg not found! Clip extraction will not work.")

# Find ffmpeg dependencies (libav*, libsw*)
_ffmpeg_libs = []
if _ffmpeg_path:
    for pattern in [
        "/opt/homebrew/lib/libav*.dylib",
        "/opt/homebrew/lib/libsw*.dylib",
        "/usr/local/lib/libav*.dylib",
        "/usr/local/lib/libsw*.dylib",
    ]:
        _ffmpeg_libs.extend(glob.glob(pattern))

_binaries = []
# Add libmpv
for lib in sorted(set(_mpv_libs)):
    real = os.path.realpath(lib)
    if real not in [b[0] for b in _binaries]:
        _binaries.append((real, os.path.basename(lib)))

# Add ffmpeg binary
if _ffmpeg_path:
    _binaries.append((_ffmpeg_path, "."))  # Use "." to place at root of bundle
    print(f"Adding ffmpeg binary to bundle: {_ffmpeg_path}")

# Add ffmpeg dependencies
for lib in sorted(set(_ffmpeg_libs)):
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
    [],
    exclude_binaries=True,
    name="jorja-clipper",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    console=False,
    disable_windowed_traceback=False,
    icon=None,
)

coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=True,
    upx_exclude=[],
    name="jorja-clipper",
)

app = BUNDLE(
    coll,
    name="Jorja Clipper.app",
    icon=None,
    bundle_identifier="com.cortexuvula.jorja-clipper",
)
