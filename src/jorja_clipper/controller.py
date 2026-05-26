"""Application controller — separates business logic from the GUI."""

import logging
from pathlib import Path

from PySide6.QtCore import QObject

from jorja_clipper.batch_queue import BatchWorker, ClipQueue, ClipRequest
from jorja_clipper.clip_store import ClipStore, StoredClip
from jorja_clipper.clipper import Clipper, ClipResult
from jorja_clipper.gui.clip_list import ClipListModel
from jorja_clipper.player import Player
from jorja_clipper.plugins import PluginLoader
from jorja_clipper.settings import Settings
from jorja_clipper.worker import ClipWorker

logger = logging.getLogger(__name__)

__all__ = ["ClipController"]


class ClipController(QObject):
    """Orchestrates clip workflow, player state, and settings."""

    def __init__(
        self,
        player: Player,
        clipper: Clipper,
        settings: Settings,
        clip_model: ClipListModel,
        clip_store: ClipStore | None = None,
        plugin_loader: PluginLoader | None = None,
    ) -> None:
        super().__init__()
        self._player = player
        self._clipper = clipper
        self._settings = settings
        self._clip_model = clip_model
        self._clip_store = clip_store or ClipStore()
        self._plugin_loader = plugin_loader or PluginLoader()
        self._current_video: Path | None = None
        self._clip_count = 0
        self._active_worker: ClipWorker | None = None
        self._last_undo_info: tuple[StoredClip, float] | None = None
        """(stored_clip, video_position_at_undo) so we can restore position on undo."""

        # Batch queue
        self._batch_queue = ClipQueue()
        self._batch_worker: BatchWorker | None = None

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

    @property
    def batch_queue(self) -> ClipQueue:
        return self._batch_queue

    @property
    def is_batch_running(self) -> bool:
        return self._batch_worker is not None and self._batch_worker.isRunning()

    @property
    def plugin_loader(self) -> PluginLoader:
        return self._plugin_loader

    # ------------------------------------------------------------------
    # File / Player operations
    # ------------------------------------------------------------------

    def load_clips_for_current_video(self) -> None:
        """Load persisted clips for the current video into the model."""
        if self._current_video is None:
            return
        stored = self._clip_store.get_clips_for_video(str(self._current_video))
        # stored is newest-first from DB, so iterate as-is to add newest last
        # (which puts it at the top of the list view)
        for sc in stored:
            self._clip_model.add_clip(sc.clip_path, sc.start_time, sc.end_time)
        self._clip_count = self._clip_model.rowCount()
        logger.debug(
            "Loaded %d persisted clip(s) for %s",
            len(stored),
            self._current_video,
        )

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
    # Single clip workflow (async)
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

        start, end = self._clipper.calculate_times(
            self._player.current_pos, self._player.duration
        )
        self._plugin_loader.broadcast_clip_start(self._current_video, start, end)

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
            self._plugin_loader.broadcast_clip_complete(result)
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
            self._plugin_loader.broadcast_clip_error(result)
            logger.error("Clip failed: %s", result.error)

        if self._active_worker is not None:
            self._active_worker.deleteLater()
            self._active_worker = None

    # ------------------------------------------------------------------
    # Batch clip queue
    # ------------------------------------------------------------------

    def queue_clip(self) -> ClipResult | None:
        """Add a clip request to the batch queue without processing it yet.

        Returns ``None`` on success, or a :class:`ClipResult` with
        ``success=False`` if there is no current video.
        """
        if self._current_video is None:
            return ClipResult(
                path="",
                start_time=0.0,
                end_time=0.0,
                success=False,
                error="No video loaded",
            )
        # Use queue length + existing clip count for numbering
        # Don't increment _clip_count yet - that happens when clip is saved
        clip_number = self._clip_count + len(self._batch_queue) + 1
        request = ClipRequest(
            video_path=self._current_video,
            current_pos=self._player.current_pos,
            video_duration=self._player.duration,
            clip_number=clip_number,
        )
        self._batch_queue.enqueue(request)
        logger.info(
            "Queued clip %d at %.1fs (queue size=%d)",
            request.clip_number,
            request.current_pos,
            len(self._batch_queue),
        )
        return None

    def process_batch(self) -> BatchWorker | ClipResult:
        """Start a :class:`BatchWorker` that drains the current queue.

        Returns the started ``BatchWorker`` so callers can connect to
        its ``progress`` and ``finished`` signals, **or** a
        :class:`ClipResult` with ``success=False`` if the queue is empty
        or a batch is already running.
        """
        if self.is_batch_running:
            return ClipResult(
                path="",
                start_time=0.0,
                end_time=0.0,
                success=False,
                error="Batch already in progress",
            )
        if len(self._batch_queue) == 0:
            return ClipResult(
                path="",
                start_time=0.0,
                end_time=0.0,
                success=False,
                error="Batch queue is empty",
            )

        worker = BatchWorker(
            clipper=self._clipper,
            queue=self._batch_queue,
            parent=None,
        )
        worker.progress.connect(self._on_batch_progress)
        worker.finished.connect(self._on_batch_finished)
        self._batch_worker = worker
        worker.start()
        return worker

    def _on_batch_progress(
        self, completed: int, total: int, result: ClipResult
    ) -> None:
        """Handle a single item finishing inside a batch run."""
        if result.success:
            self._clip_model.add_clip(result.path, result.start_time, result.end_time)
            if self._current_video is not None:
                self._clip_store.add_clip(
                    clip_path=result.path,
                    source_video_path=str(self._current_video),
                    start_time=result.start_time,
                    end_time=result.end_time,
                )
            self._plugin_loader.broadcast_clip_complete(result)
            logger.info(
                "Batch progress %d/%d — saved %s", completed, total, result.path
            )
        else:
            self._plugin_loader.broadcast_clip_error(result)
            logger.error(
                "Batch progress %d/%d — failed: %s", completed, total, result.error
            )

    def _on_batch_finished(self, results: list[ClipResult]) -> None:
        """Handle completion of the entire batch queue."""
        success_count = sum(1 for r in results if r.success)
        logger.info("Batch finished: %d/%d succeeded", success_count, len(results))
        if self._batch_worker is not None:
            self._batch_worker.deleteLater()
            self._batch_worker = None

    def clear_batch_queue(self) -> None:
        """Remove all pending requests from the batch queue."""
        self._batch_queue.clear()
        logger.info("Batch queue cleared")

    # ------------------------------------------------------------------
    # Undo
    # ------------------------------------------------------------------

    def undo_last_clip(self) -> bool:
        """Undo the most recent clip: delete file, DB entry, model, restore position.

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
