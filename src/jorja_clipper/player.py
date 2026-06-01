"""Player wrapper around python-mpv."""

import sys
from pathlib import Path


class Player:
    """Wraps mpv for video playback with clean interface.

    On macOS, mpv's cocoa VO ignores wid and always opens its own window.
    We use vo=libmpv with an OpenGL render context managed by VideoWidget.
    """

    def __init__(self):
        self._mpv = None
        self._duration = 0.0
        self._current_pos = 0.0
        self._paused = True
        self._wid = None
        self._gl_widget = None
        self._render_ctx = None

    def init_with_wid(self, wid: int, gl_widget=None) -> None:
        """Store the widget reference. mpv init happens in VideoWidget.initializeGL()."""
        self._wid = wid
        self._gl_widget = gl_widget

    @property
    def duration(self) -> float:
        return self._duration

    @property
    def current_pos(self) -> float:
        return self._current_pos

    @property
    def paused(self) -> bool:
        return self._paused

    def load(self, path: Path) -> None:
        """Load a video file. mpv is initialized by VideoWidget.initializeGL()."""
        if self._mpv is None:
            return
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
        if self._gl_widget is not None:
            self._gl_widget.cleanup()
        if self._mpv is not None:
            self._mpv.terminate()
            self._mpv = None
