"""PyInstaller runtime hook: make python-mpv find bundled libmpv."""

import ctypes.util
import os
import sys

_orig_find_library = ctypes.util.find_library


def _patched_find_library(name):
    """Search the PyInstaller bundle dir before falling back to system paths."""
    # Check bundle first (for packaged apps)
    if name in ("mpv", "libmpv"):
        bundle_dir = getattr(sys, "_MEIPASS", None)
        if bundle_dir:
            candidates = [
                "libmpv.so",
                "libmpv.so.2",
                "libmpv.dylib",
                "libmpv.2.dylib",
                "mpv.dll",
                "mpv-1.dll",
                "mpv-2.dll",
                "libmpv-2.dll",
                # PyInstaller sometimes creates __dot__ escaped directories
                "libmpv__dot__2__dot__dylib/libmpv.2.dylib",
            ]
            for candidate in candidates:
                path = os.path.join(bundle_dir, candidate)
                if os.path.isfile(path):
                    return path

    # Fall back to system paths
    return _orig_find_library(name)


ctypes.util.find_library = _patched_find_library
