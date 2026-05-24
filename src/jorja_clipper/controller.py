"""Application controller — separates business logic from the GUI."""

import logging
from pathlib import Path

from jorja_clipper.clipper import Clipper, ClipResult
from jorja_clipper.gui.clip_list import ClipListModel
from jorja_clipper.player import Player
from jorja_clipper.settings import Settings
from jorja_clipper.worker import ClipWorker

logger = logging.getLogger(__name__)


class ClipController:
    """Orchestrates clip workflow, player state, and settings."""

    def __init__(
        self,
        player: Player,
        clipper: Clipper,
        settings: Settings,
        clip_model: ClipListModel,
    ):
        self._player = player
        self._clipper = clipper
        self._settings = settings
        self._clip_model = clip_model
        self._current_video: Path | None = None
        self._clip_count = 0
        self._active_worker: ClipWorker | None = None

    # ------------------------------------------------------------------
    # Properties delegated to underlying components
    # ------------------------------------------------------------------

    @property
    def player(self) -> Player:
        return self._player

    @property
    def settings(self) -> Settings:
        return self._settings

    @property
    def clip_model(self) -> ClipListModel:
        return self._clip_model

    @property
    def current_video(self) -> Path | None:
        return self._current_video

    @property
    def clip_count(self) -> int:
        return self._clip_count

    @property
    def is_clipping(self) -> bool:
        return self._active_worker is not None and self._active_worker.isRunning()

    # ------------------------------------------------------------------
    # File / Player operations
    # ------------------------------------------------------------------

    def open_file(self, video_path: Path) -> bool:
        """Load a video file into the player."""
        logger.info("Opening video: %s", video_path)
        self._current_video = video_path
        success = self._player.load(video_path)
        if success:
            logger.info("Video loaded successfully")
        else:
            logger.error("Failed to load video: %s", video_path)
        return success

    def toggle_play(self) -> None:
        """Toggle play / pause."""
        self._player.toggle_pause()

    def seek(self, offset: float) -> None:
        """Seek by relative offset in seconds."""
        self._player.seek(offset)

    def shutdown(self) -> None:
        """Clean up the player on application exit."""
        self._player.shutdown()

    # ------------------------------------------------------------------
    # Clip workflow (async)
    # ------------------------------------------------------------------

    def save_clip(self) -> ClipWorker | ClipResult:
        """Start an async clip save.

        Returns the started ``ClipWorker`` so callers can connect to its
        ``finished`` signal, **or** a ``ClipResult`` with ``success=False``
        if the request was rejected (no video loaded or worker already
        running).
        """
        if self.is_clipping:
            logger.warning("Clip request ignored — worker already running")
            return ClipResult(
                path="",
                start_time=0.0,
                end_time=0.0,
                success=False,
                error="Clip already in progress",
            )

        if self._current_video is None:
            logger.warning("save_clip called with no current video")
            return ClipResult(
                path="",
                start_time=0.0,
                end_time=0.0,
                success=False,
                error="No video loaded",
            )

        logger.info(
            "Starting clip worker at %.1fs (video=%s)",
            self._player.current_pos,
            self._current_video.name,
        )

        worker = ClipWorker(
            clipper=self._clipper,
            video_path=self._current_video,
            current_pos=self._player.current_pos,
            video_duration=self._player.duration,
            clip_number=self._clip_count + 1,
            parent=None,
        )
        worker.finished.connect(self._on_clip_finished)
        self._active_worker = worker
        worker.start()
        return worker

    def _on_clip_finished(self, result: ClipResult) -> None:
        """Handle completion of a clip worker thread."""
        if result.success:
            self._clip_count += 1
            self._clip_model.add_clip(result.path, result.start_time, result.end_time)
            logger.info("Clip saved: %s", result.path)
        else:
            logger.error("Clip failed: %s", result.error)

        if self._active_worker is not None:
            self._active_worker.deleteLater()
            self._active_worker = None

    # ------------------------------------------------------------------
    # Settings operations
    # ------------------------------------------------------------------

    def apply_settings(self) -> None:
        """Propagate updated settings values to components."""
        logger.debug(
            "Applying settings: before=%.1fs after=%.1fs key=%s",
            self._settings.buffer_before,
            self._settings.buffer_after,
            self._settings.clip_key,
        )
        self._clipper.buffer_before = self._settings.buffer_before
        self._clipper.buffer_after = self._settings.buffer_after
