# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for Linux — bundles libmpv."""

import glob
import os

from common import make_analysis, make_exe, make_pyz

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

a = make_analysis("../src/jorja_clipper/app.py", binaries=_binaries)

pyz = make_pyz(a)

exe = make_exe(pyz, a, icon=None, console=False)
