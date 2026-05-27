"""Player wrapper around python-mpv."""

import contextlib
import logging
import os
import sys
import threading
from pathlib import Path
from typing import Any

import mpv

logger = logging.getLogger(__name__)

__all__ = ["Player"]


class Player:
    """Wraps mpv for video playback with clean interface."""

    def __init__(self) -> None:
        self._mpv: mpv.MPV | None = None
        self._duration = 0.0
        self._current_pos = 0.0
        self._paused = True
        self._wid: int | None = None
        self._lock = threading.Lock()
        self._property_refs: list[Any] = []  # prevent GC of observer closures

    def _ensure_mpv(self) -> None:
        if self._mpv is not None:
            return
        kwargs: dict[str, object] = {
            "input_default_bindings": False,
            "input_vo_keyboard": False,
            "osc": False,
        }
        if sys.platform.startswith("darwin"):
            # macOS: use libmpv render API via MpvRenderContext.
            # The widget creates the render context after GL init.
            kwargs["vo"] = "libmpv"
        if self._wid is not None:
            kwargs["wid"] = self._wid
        logger.info(
            "Creating mpv instance (vo=%s, wid=%s)", kwargs.get("vo", "auto"), self._wid
        )
        # Re-apply locale fix right before mpv init — Qt resets LC_NUMERIC
        # on Linux, and libmpv crashes if it isn't "C".
        import locale

        with contextlib.suppress(locale.Error):
            locale.setlocale(locale.LC_NUMERIC, "C")

        # On Linux with XWayland (QT_QPA_PLATFORM=xcb), mpv may detect the
        # Wayland display and create its own Wayland window, ignoring the X11
        # --wid parameter. Temporarily unset WAYLAND_DISPLAY to force mpv to
        # use X11 and respect the wid embedding.
        saved_wayland_display = None
        if self._wid is not None and sys.platform == "linux":
            saved_wayland_display = os.environ.pop("WAYLAND_DISPLAY", None)

        try:
            self._mpv = mpv.MPV(**kwargs)
        finally:
            # Restore WAYLAND_DISPLAY for other components
            if saved_wayland_display is not None:
                os.environ["WAYLAND_DISPLAY"] = saved_wayland_display

        # Register property observers — keep strong references to prevent GC
        # while mpv's event thread may still invoke them.
        def _on_duration(_name: str, value: Any) -> None:
            if value is not None:
                with self._lock:
                    self._duration = float(value)  # type: ignore[arg-type]

        def _on_time_pos(_name: str, value: Any) -> None:
            if value is not None:
                with self._lock:
                    self._current_pos = float(value)  # type: ignore[arg-type]

        def _on_pause(_name: str, value: Any) -> None:
            if value is not None:
                with self._lock:
                    self._paused = bool(value)

        self._mpv.property_observer("duration")(_on_duration)
        self._mpv.property_observer("time-pos")(_on_time_pos)
        self._mpv.property_observer("pause")(_on_pause)

        # Prevent GC of closures while mpv is alive
        self._property_refs = [_on_duration, _on_time_pos, _on_pause]

    def init_with_wid(self, wid: int) -> None:
        """Bind mpv to a native widget handle (lazy init). Used on Linux/Windows."""
        self._wid = wid

    @property
    def mpv_handle(self) -> mpv.MPV | None:
        """Return the underlying mpv instance (for render context creation)."""
        self._ensure_mpv()
        return self._mpv

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
        with self._lock:
            return self._paused

    def load(self, path: Path) -> bool:
        """Load a video file."""
        self._ensure_mpv()
        try:
            self._mpv.play(str(path))
        except Exception:
            logger.exception("mpv.play() failed for %s", path)
            return False
        try:
            self._mpv.pause = "yes"
        except Exception:
            logger.exception("Failed to set initial pause state")
        m = self._mpv
        with self._lock:
            self._paused = bool(m.pause) if m is not None else True
        return True

    def toggle_pause(self) -> None:
        """Toggle play / pause."""
        if self._mpv is None:
            return
        with self._lock:
            new_state = not self._paused
        self._mpv.pause = "yes" if new_state else "no"
        # The property observer updates self._paused when mpv acknowledges
        # the change, so we don't write it here directly.

    def seek(self, offset: float) -> None:
        """Seek by relative offset in seconds."""
        if self._mpv is None:
            return
        with contextlib.suppress(Exception):
            self._mpv.command("seek", offset, "relative")

    def seek_to(self, position: float) -> None:
        """Seek to an absolute position in seconds."""
        if self._mpv is None:
            return
        with contextlib.suppress(Exception):
            self._mpv.command("seek", position, "absolute")

    def shutdown(self) -> None:
        """Clean up mpv instance."""
        if self._mpv is not None:
            self._property_refs.clear()
            self._mpv.terminate()
            self._mpv = None
