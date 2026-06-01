"""Player wrapper around python-mpv."""

import ctypes.util
import sys
from pathlib import Path

# Patch ctypes.util.find_library to check Homebrew paths on Apple Silicon macOS
if sys.platform == "darwin":
    _original_find_library = ctypes.util.find_library

    def _patched_find_library(name):
        result = _original_find_library(name)
        if result is None and name == "mpv":
            # Check Apple Silicon and Intel Homebrew paths
            for lib_path in ["/opt/homebrew/lib/libmpv.dylib", "/usr/local/lib/libmpv.dylib"]:
                if Path(lib_path).exists():
                    return lib_path
        return result

    ctypes.util.find_library = _patched_find_library

import mpv


class Player:
    """Wraps mpv for video playback with clean interface."""

    def __init__(self):
        self._mpv = None
        self._duration = 0.0
        self._current_pos = 0.0
        self._paused = True
        self._wid = None

    def _ensure_mpv(self):
        if self._mpv is not None:
            return
        kwargs = {
            "input_default_bindings": False,
            "input_vo_keyboard": False,
            "osc": False,
        }
        if sys.platform.startswith("darwin"):
            # macOS: libmpv + NSView via wid option.
            kwargs["vo"] = "libmpv"
        if self._wid is not None:
            kwargs["wid"] = self._wid
        self._mpv = mpv.MPV(**kwargs)

        @self._mpv.property_observer("duration")
        def _on_duration(_name, value):
            if value is not None:
                self._duration = float(value)

        @self._mpv.property_observer("time-pos")
        def _on_time_pos(_name, value):
            if value is not None:
                self._current_pos = float(value)

    def init_with_wid(self, wid: int) -> None:
        """Bind mpv to a native widget handle (lazy init)."""
        self._wid = wid

    @property
    def duration(self) -> float:
        """Total video duration in seconds."""
        return self._duration

    @property
    def current_pos(self) -> float:
        """Current playback position in seconds."""
        return self._current_pos

    @property
    def paused(self) -> bool:
        """Whether playback is paused."""
        return self._paused

    def load(self, path: Path) -> None:
        """Load a video file."""
        self._ensure_mpv()
        self._mpv.play(str(path))
        self._mpv.pause = "yes"
        self._paused = True

    def toggle_pause(self) -> None:
        """Toggle play/pause."""
        if self._mpv is None:
            return
        self._paused = not self._paused
        self._mpv.pause = "yes" if self._paused else "no"

    def seek(self, offset: float) -> None:
        """Seek by relative offset in seconds."""
        if self._mpv is None:
            return
        self._mpv.command("seek", offset, "relative")

    def shutdown(self) -> None:
        """Clean up mpv instance."""
        if self._mpv is not None:
            self._mpv.terminate()
            self._mpv = None
