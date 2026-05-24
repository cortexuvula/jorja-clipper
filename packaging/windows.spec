# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for Windows — bundles mpv DLLs."""

import glob
import os

from common import make_analysis, make_exe, make_pyz

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

a = make_analysis("..\\src\\jorja_clipper\\app.py", binaries=_binaries)

pyz = make_pyz(a)

exe = make_exe(pyz, a, icon=None, console=False)
