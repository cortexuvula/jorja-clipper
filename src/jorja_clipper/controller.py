"""Application controller — separates business logic from the GUI."""

import logging
from pathlib import Path

from jorja_clipper.clipper import Clipper, ClipResult
from jorja_clipper.clip_store import ClipStore, StoredClip
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
        clip_store: ClipStore | None = None,
    ) -> None:
        self._player = player
        self._clipper = clipper
        self._settings = settings
        self._clip_model = clip_model
        self._clip_store = clip_store or ClipStore()
        self._current_video: Path | None = None
        self._clip_count = 0
        self._active_worker: ClipWorker | None = None
        self._last_undo_info: tuple[StoredClip, float] | None = None
        """(stored_clip, video_position_at_undo) so we can restore position on undo."""

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

    def load_clips_for_current_video(self) -> None:
        """Load persisted clips for the current video into the model."""
        if self._current_video is None:
            return
        stored = self._clip_store.get_clips_for_video(str(self._current_video))
        for sc in reversed(stored):  # oldest first so newest ends up on top
            self._clip_model.add_clip(sc.clip_path, sc.start_time, sc.end_time)
        self._clip_count = self._clip_model.rowCount()
        logger.debug("Loaded %d persisted clip(s) for %s", len(stored), self._current_video)

    def open_file(self, video_path: Path) -> bool:
        """Load a video file into the player."""
        logger.info("Opening video: %s", video_path)
        self._current_video = video_path
        success = self._player.load(video_path)
        if success:
            logger.info("Video loaded successfully")
            self.load_clips_for_current_video()
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
            if self._current_video is not None:
                self._clip_store.add_clip(
                    clip_path=result.path,
                    source_video_path=str(self._current_video),
                    start_time=result.start_time,
                    end_time=result.end_time,
                )
                stored = self._clip_store.get_last_clip()
                if stored is not None:
                    self._last_undo_info = (stored, result.start_time)
            logger.info("Clip saved: %s", result.path)
        else:
            logger.error("Clip failed: %s", result.error)

        if self._active_worker is not None:
            self._active_worker.deleteLater()
            self._active_worker = None

    def undo_last_clip(self) -> bool:
        """Undo the most recent clip: delete file, DB entry, model row, and restore position.

        Returns True if an undo was performed.
        """
        if self._last_undo_info is None:
            logger.info("Undo requested but no clip to undo")
            return False

        stored, restore_pos = self._last_undo_info
        logger.info("Undoing clip %d at %s", stored.id, stored.clip_path)

        # 1. Remove from model (remove the last/top row)
        self._clip_model.remove_last()
        self._clip_count = max(0, self._clip_count - 1)

        # 2. Delete from persistence
        self._clip_store.delete_clip(stored.id)

        # 3. Delete file from disk
        try:
            Path(stored.clip_path).unlink(missing_ok=True)
        except OSError as exc:
            logger.warning("Could not delete clip file: %s", exc)

        # 4. Restore video position
        self._player.seek_to(restore_pos)

        self._last_undo_info = None
        logger.info("Undo complete — restored position %.1fs", restore_pos)
        return True

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
