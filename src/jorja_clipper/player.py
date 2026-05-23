"""Player wrapper around python-mpv."""

from pathlib import Path

import mpv


class Player:
    """Wraps mpv for video playback with clean interface."""

    def __init__(self):
        self._mpv = mpv.MPV(
            input_default_bindings=False,
            input_vo_keyboard=False,
            osc=False,
        )
        self._duration = 0.0
        self._current_pos = 0.0
        self._paused = True

        @self._mpv.property_observer("duration")
        def _on_duration(_name, value):
            if value is not None:
                self._duration = float(value)

        @self._mpv.property_observer("time-pos")
        def _on_time_pos(_name, value):
            if value is not None:
                self._current_pos = float(value)

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
        self._mpv.play(str(path))
        self._mpv.pause = "yes"
        self._paused = True

    def toggle_pause(self) -> None:
        """Toggle play/pause."""
        self._paused = not self._paused
        self._mpv.pause = "yes" if self._paused else "no"

    def seek(self, offset: float) -> None:
        """Seek by relative offset in seconds."""
        self._mpv.command("seek", offset, "relative")

    def shutdown(self) -> None:
        """Clean up mpv instance."""
        self._mpv.terminate()
