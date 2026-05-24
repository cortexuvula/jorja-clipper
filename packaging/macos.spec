# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for macOS — bundles libmpv."""

import glob
import os

from common import make_analysis, make_exe, make_pyz

# Find libmpv.dylib — installed by `brew install mpv`
_mpv_libs = []
for pattern in [
    "/opt/homebrew/lib/libmpv*.dylib",   # Apple Silicon
    "/usr/local/lib/libmpv*.dylib",      # Intel
]:
    _mpv_libs.extend(glob.glob(pattern))

_binaries = []
for lib in sorted(set(_mpv_libs)):
    real = os.path.realpath(lib)
    if real not in [b[0] for b in _binaries]:
        _binaries.append((real, os.path.basename(lib)))

a = make_analysis("../src/jorja_clipper/app.py", binaries=_binaries)

pyz = make_pyz(a)

exe, coll = make_exe(pyz, a, icon=None, console=False, collect=True)

app = BUNDLE(
    coll,
    name="Jorja Clipper.app",
    icon=None,
    bundle_identifier="com.cortexuvula.jorja-clipper",
)
