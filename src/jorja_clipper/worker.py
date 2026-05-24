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

    def run(self) -> None:
        """Execute the blocking ffmpeg call and emit the result."""
        result = self._clipper.save_clip(
            video_path=self._video_path,
            current_pos=self._current_pos,
            video_duration=self._video_duration,
            clip_number=self._clip_number,
        )
        self.finished.emit(result)
