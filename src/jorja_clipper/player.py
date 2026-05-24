"""Player wrapper around python-mpv."""

import sys
import threading
from pathlib import Path

import mpv


class Player:
    """Wraps mpv for video playback with clean interface."""

    def __init__(self):
        self._mpv = None
        self._duration = 0.0
        self._current_pos = 0.0
        self._paused = True
        self._wid = None
        self._lock = threading.Lock()

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
                with self._lock:
                    self._duration = float(value)

        @self._mpv.property_observer("time-pos")
        def _on_time_pos(_name, value):
            if value is not None:
                with self._lock:
                    self._current_pos = float(value)

        @self._mpv.property_observer("pause")
        def _on_pause(_name, value):
            if value is not None:
                self._paused = bool(value)

    def init_with_wid(self, wid: int) -> None:
        """Bind mpv to a native widget handle (lazy init)."""
        self._wid = wid

    @property
    def duration(self) -> float:
        """Total video duration in seconds."""
        with self._lock:
            return self._duration

    @property
    def current_pos(self) -> float:
        """Current playback position in seconds."""
        with self._lock:
            return self._current_pos

    @property
    def paused(self) -> bool:
        """Whether playback is paused."""
        return self._paused

    def load(self, path: Path) -> bool:
        """Load a video file."""
        self._ensure_mpv()
        try:
            self._mpv.play(str(path))
        except mpv.MPVError as exc:
            return False
        self._mpv.pause = "yes"
        self._paused = bool(self._mpv.pause) if self._mpv is not None else True
        return True

    def toggle_pause(self) -> None:
        """Toggle play/pause."""
        if self._mpv is None:
            return
        new_state = not self._paused
        self._mpv.pause = "yes" if new_state else "no"
        self._paused = new_state

    def seek(self, offset: float) -> None:
        """Seek by relative offset in seconds."""
        if self._mpv is None:
            return
        try:
            self._mpv.command("seek", offset, "relative")
        except mpv.MPVError:
            pass

    def shutdown(self) -> None:
        """Clean up mpv instance."""
        if self._mpv is not None:
            self._mpv.terminate()
            self._mpv = None
