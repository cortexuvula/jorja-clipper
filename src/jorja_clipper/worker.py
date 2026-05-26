"""Async clip worker — runs ffmpeg in a background QThread."""

from pathlib import Path

from PySide6.QtCore import QObject, QThread, Signal

from jorja_clipper.clipper import Clipper

__all__ = ["ClipWorker"]


class ClipWorker(QThread):
    """Runs clip extraction in a background thread so the Qt UI stays responsive."""

    finished = Signal(object)

    def __init__(
        self,
        clipper: Clipper,
        video_path: Path,
        current_pos: float,
        video_duration: float,
        clip_number: int,
        parent: QObject | None = None,
    ) -> None:
        super().__init__(parent)
        self._clipper = clipper
        self._video_path = video_path
        self._current_pos = current_pos
        self._video_duration = video_duration
        self._clip_number = clip_number
        self._cancelled = False

    def cancel(self) -> None:
        """Request cancellation of this worker.

        Sets an internal flag that ``run()`` checks before starting the
        ffmpeg subprocess and before emitting the result signal.  The
        subprocess itself has a 30 s timeout and will finish on its own;
        cancellation prevents the result from being delivered to a
        potentially torn-down UI.
        """
        self._cancelled = True

    def _is_cancelled(self) -> bool:
        """Check both the custom flag and Qt's interruption request."""
        return self._cancelled or self.isInterruptionRequested()

    def run(self) -> None:
        """Execute the blocking ffmpeg call and emit the result.

        Checks for cancellation before starting and before emitting the
        result so that a shutdown can suppress the signal (the ffmpeg
        subprocess has its own 30 s timeout and will finish on its own,
        but we avoid delivering results to a torn-down UI).
        """
        if self._is_cancelled():
            return
        result = self._clipper.save_clip(
            video_path=self._video_path,
            current_pos=self._current_pos,
            video_duration=self._video_duration,
            clip_number=self._clip_number,
        )
        if not self._is_cancelled():
            self.finished.emit(result)
